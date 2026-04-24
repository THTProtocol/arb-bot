use clap::Parser;
pub mod cli;
pub mod ntp_check;
pub mod shutdown;

use arb_adapters::{binance::BinanceAdapter, kraken::KrakenAdapter, okx::OkxAdapter};
use arb_config::loader::load;
use arb_core::types::{BookUpdate, ExchangeAdapter, NormalizedSymbol, Opportunity, Venue};
use arb_engine::engine::{Engine, EngineConfig};
use arb_exec::paper_executor::PaperExecutor;
use arb_log::logger::OpportunityLogger;
use arb_metrics::exporter::{start, Metrics};
use flume::unbounded;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .with_target(false)
        .init();

    if let Err(e) = ntp_check::check(&args.config).await {
        error!("NTP check failed: {}", e);
        std::process::exit(1);
    }

    let mut cfg = load(Path::new(&args.config))?;
    info!("Loaded config: mode={:?}", cfg.mode);

    if let Some(ref mode_str) = args.mode {
        cfg.mode = match mode_str.to_lowercase().as_str() {
            "paper" => arb_core::types::BuildMode::Paper,
            "observe" => arb_core::types::BuildMode::Observe,
            "live" => arb_core::types::BuildMode::Live,
            other => {
                anyhow::bail!("unknown mode: {}", other);
            }
        };
    }

    if let Some(ref cli_venues) = args.venues {
        cfg.venues.enabled = cli_venues.clone();
        info!("Venues overridden by CLI: {:?}", cfg.venues.enabled);
    }

    let enabled = cfg.enabled_venues();

    // Telegram notifier
    let tg: Option<std::sync::Arc<arb_telegram::TelegramNotifier>> =
        args.tg_bot_token.clone().zip(args.tg_chat_id.clone()).map(|(tok, cid)| {
            std::sync::Arc::new(arb_telegram::TelegramNotifier::new(tok, cid))
        });

    if let Some(ref tg) = tg {
        tg.online(
            &enabled.iter().map(|v| format!("{}", v)).collect::<Vec<_>>(),
            &format!("{:?}", cfg.mode),
        )
        .await;
    }

    // TRAIN MODE — sweep hyperparameters over recorded data
    if let Some(ref train_input) = args.train_input {
        let grid = arb_train::grid::Grid {
            profit_threshold_bps: vec![5.0, 10.0, 20.0],
            notional_usd: vec![100.0, 500.0, 1000.0],
            usd_usdt_basis_bps: vec![0.0, 1.0],
            slippage_alpha: vec![0.01, 0.05, 0.1],
            strategies: vec![
                vec!["taker_taker".into()],
                vec!["maker_taker".into()],
                vec!["taker_taker".into(), "maker_taker".into()],
            ],
        };
        info!("Training mode: {} combos", grid.combos().len());
        let mut results = Vec::new();
        for combo in grid.combos() {
            let r = arb_train::sweep::run_sweep(
                train_input,
                &combo,
                cfg.symbol_maps(),
                enabled.clone(),
            )
            .await?;
            info!(
                "Combo {:?} = opps={} avg_net={:.2} sharpe={:.2}",
                combo, r.num_opps, r.avg_net_bps, r.sharpe
            );
            results.push(r);
        }
        let out = args
            .train_output
            .clone()
            .unwrap_or_else(|| "train_report.csv".into());
        arb_train::report::write_csv(&out, &results)?;
        info!("Training report written to {}", out);
        return Ok(());
    }

    if enabled.len() < 2 {
        anyhow::bail!("arbitrage requires at least 2 venues enabled");
    }

    // LIVE MODE — use real order clients
    let live_executor: Option<std::sync::Arc<arb_live::live_executor::LiveExecutor>> =
        if cfg.mode == arb_core::types::BuildMode::Live {
            info!("LIVE mode active — using real order endpoints");
            let keys = cfg.api_keys();
            let binance_client = if enabled.contains(&Venue::Binance) {
                keys.binance_api_key
                    .zip(keys.binance_api_secret)
                    .map(|(k, s)| arb_live::binance_orders::BinanceClient::new(k, s))
            } else {
                None
            };
            let okx_client = if enabled.contains(&Venue::Okx) {
                keys.okx_api_key
                    .zip(keys.okx_api_secret)
                    .zip(keys.okx_passphrase)
                    .map(|((k, s), p)| arb_live::okx_orders::OkxClient::new(k, s, p))
            } else {
                None
            };
            let executor = arb_live::live_executor::LiveExecutor::new(binance_client, okx_client);
            Some(std::sync::Arc::new(executor))
        } else {
            None
        };

    let (book_tx, book_rx) = unbounded::<BookUpdate>();
    let (opp_tx, opp_rx) = unbounded::<Opportunity>();

    let metrics = Arc::new(Metrics::new());
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = start(metrics_clone, cfg.metrics.port).await {
            error!("Metrics server error: {}", e);
        }
    });

    let engine_cfg = EngineConfig {
        mode: cfg.mode.clone(),
        profit_threshold_bps: cfg.profit_threshold_bps,
        notional_usd: cfg.notional_usd,
        usd_usdt_basis_bps: cfg.usd_usdt_basis_bps,
        strategies: cfg.strategies.clone(),
        symbol_maps: cfg.symbol_maps(),
        enabled_venues: enabled.clone(),
        fee_model: cfg.fee_model(),
        slippage_alpha: 0.05,
    };

    let engine = Engine::new(engine_cfg, opp_tx.clone());
    tokio::spawn(async move {
        engine.run(book_rx).await;
    });

    let logger = OpportunityLogger::new(Path::new(&args.output));
    let logger_clone = logger.clone();
    let live_executor_clone = live_executor.clone();
    let tg_clone = tg.clone();
    tokio::spawn(async move {
        while let Ok(opp) = opp_rx.recv_async().await {
            let _ = logger_clone.log(&opp).await;
            if let Some(ref exec) = live_executor_clone {
                match exec.execute(&opp).await {
                    Ok(fill) => {
                        if let Some(ref tg) = tg_clone {
                            tg.trade(
                                &fill.symbol,
                                &fill.direction,
                                fill.pnl,
                                fill.buy_avg_px,
                                fill.sell_avg_px,
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Live execution failed: {}", e);
                    }
                }
            }
        }
    });

    let mut _executor = PaperExecutor::new(
        &args
            .ledger
            .clone()
            .unwrap_or_else(|| "paper_ledger.jsonl".into()),
    );

    for &venue in &enabled {
        match venue {
            Venue::Binance => {
                let binance_symbols: Vec<NormalizedSymbol> = cfg
                    .symbols
                    .iter()
                    .filter(|s| s.binance.is_some())
                    .map(|s| {
                        let b = s.binance.as_ref().unwrap();
                        let base = b[..b.len().saturating_sub(4)].to_string();
                        let quote = b[b.len().saturating_sub(4)..].to_string().to_uppercase();
                        NormalizedSymbol::new(base, quote)
                    })
                    .collect();
                let binance = BinanceAdapter::new(binance_symbols);
                let book_tx_clone = book_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = binance.start(book_tx_clone).await {
                        error!("Binance adapter error: {}", e);
                    }
                });
            }
            Venue::Kraken => {
                let kraken_symbols: Vec<NormalizedSymbol> = cfg
                    .symbols
                    .iter()
                    .filter(|s| s.kraken.is_some())
                    .map(|s| {
                        let k = s.kraken.as_ref().unwrap();
                        let parts: Vec<&str> = k.split('/').collect();
                        NormalizedSymbol::new(
                            parts
                                .get(0)
                                .unwrap_or(&"")
                                .to_string()
                                .replace("XBT", "BTC"),
                            parts.get(1).unwrap_or(&"").to_string(),
                        )
                    })
                    .collect();
                let kraken = KrakenAdapter::new(kraken_symbols);
                let book_tx_clone = book_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = kraken.start(book_tx_clone).await {
                        error!("Kraken adapter error: {}", e);
                    }
                });
            }
            Venue::Okx => {
                let okx_symbols: Vec<NormalizedSymbol> = cfg
                    .symbols
                    .iter()
                    .filter(|s| s.okx.is_some())
                    .map(|s| {
                        let o = s.okx.as_ref().unwrap();
                        let parts: Vec<&str> = o.split('-').collect();
                        NormalizedSymbol::new(
                            parts.get(0).unwrap_or(&"").to_string(),
                            parts.get(1).unwrap_or(&"").to_string(),
                        )
                    })
                    .collect();
                let okx = OkxAdapter::new(okx_symbols);
                let book_tx_clone = book_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = okx.start(book_tx_clone).await {
                        error!("OKX adapter error: {}", e);
                    }
                });
            }
        }
    }

    shutdown::wait().await;
    info!("Shutting down gracefully");
    Ok(())
}
