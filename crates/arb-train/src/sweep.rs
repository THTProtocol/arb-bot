use crate::grid::Combo;
use arb_book::order_book::OrderBook;
use arb_core::types::{BookUpdate, NormalizedSymbol, Opportunity, Strategy, Venue};
use arb_engine::engine::{Engine, EngineConfig};
use arb_recording::recorder::Recorder;
use flume::unbounded;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SweepResult {
    pub combo: Combo,
    pub num_opps: usize,
    pub avg_net_bps: f64,
    pub sharpe: f64,
    pub max_drawdown_bps: f64,
    pub win_rate: f64,
}

pub async fn run_sweep(
    input_path: &str,
    combo: &Combo,
    symbol_maps: Vec<HashMap<Venue, NormalizedSymbol>>,
    enabled_venues: Vec<Venue>,
) -> anyhow::Result<SweepResult> {
    let (book_tx, book_rx) = unbounded::<BookUpdate>();
    let (opp_tx, opp_rx) = unbounded::<Opportunity>();

    let strategies = combo
        .strategies
        .iter()
        .filter_map(|s| match s.as_str() {
            "taker_taker" => Some(Strategy::TakerTaker),
            "maker_taker" => Some(Strategy::MakerTaker),
            _ => None,
        })
        .collect();

    let engine_cfg = EngineConfig {
        mode: arb_core::types::BuildMode::Observe,
        profit_threshold_bps: combo.profit_threshold_bps,
        notional_usd: combo.notional_usd,
        usd_usdt_basis_bps: combo.usd_usdt_basis_bps,
        strategies,
        symbol_maps,
        enabled_venues,
        fee_model: arb_core::fee_model::FeeModel {
            binance_taker_bps: 10.0,
            binance_maker_bps: 10.0,
            kraken_taker_bps: 16.0,
            kraken_maker_bps: 10.0,
            okx_taker_bps: 10.0,
            okx_maker_bps: 8.0,
        },
        slippage_alpha: combo.slippage_alpha,
    };

    let engine = Engine::new(engine_cfg, opp_tx.clone());
    let engine_handle = tokio::spawn(async move {
        engine.run(book_rx).await;
    });

    let file = tokio::fs::File::open(input_path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);

    while let Ok(Some(line)) = lines.next_line().await {
        let raw: serde_json::Value = serde_json::from_str(&line)?;
        let venue_str = raw["venue"].as_str().unwrap_or("");
        let venue = match venue_str {
            "binance" => Venue::Binance,
            "kraken" => Venue::Kraken,
            "okx" => Venue::Okx,
            _ => continue,
        };
        let sym_parts: Vec<&str> = raw["symbol"].as_str().unwrap_or("").split('/').collect();
        let symbol = NormalizedSymbol::new(
            sym_parts.get(0).unwrap_or(&"").to_string(),
            sym_parts.get(1).unwrap_or(&"").to_string(),
        );
        let bids: Vec<(ordered_float::OrderedFloat<f64>, f64)> = raw["bids"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| {
                let arr = v.as_array()?;
                let p = arr.get(0)?.as_f64()?;
                let q = arr.get(1)?.as_f64()?;
                Some((ordered_float::OrderedFloat(p), q))
            })
            .collect();
        let asks: Vec<(ordered_float::OrderedFloat<f64>, f64)> = raw["asks"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| {
                let arr = v.as_array()?;
                let p = arr.get(0)?.as_f64()?;
                let q = arr.get(1)?.as_f64()?;
                Some((ordered_float::OrderedFloat(p), q))
            })
            .collect();
        let update = BookUpdate {
            venue,
            symbol,
            ts_ns: raw["ts_ns"].as_u64().unwrap_or(0),
            seq: raw["seq"].as_u64().unwrap_or(0),
            bids,
            asks,
            is_snapshot: raw["is_snapshot"].as_bool().unwrap_or(false),
        };
        let _ = book_tx.send_async(update).await;
    }

    drop(book_tx);
    drop(opp_tx);
    engine_handle.await?;

    let mut opps = Vec::new();
    while let Ok(opp) = opp_rx.recv_async().await {
        opps.push(opp);
    }

    let num_opps = opps.len();
    let net_vals: Vec<f64> = opps.iter().map(|o| o.net_bps).collect();
    let avg_net_bps = if num_opps > 0 {
        net_vals.iter().sum::<f64>() / num_opps as f64
    } else {
        0.0
    };

    let sharpe = if net_vals.len() > 1 {
        let mean = avg_net_bps;
        let variance =
            net_vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (net_vals.len() - 1) as f64;
        let std_dev = variance.sqrt();
        if std_dev > 0.0 {
            mean / std_dev
        } else {
            0.0
        }
    } else {
        0.0
    };

    let mut max_dd = 0.0;
    let mut peak = 0.0;
    let mut cum = 0.0;
    for v in &net_vals {
        cum += v;
        if cum > peak {
            peak = cum;
        }
        let dd = peak - cum;
        if dd > max_dd {
            max_dd = dd;
        }
    }

    let win_count = net_vals.iter().filter(|v| **v > 0.0).count();
    let win_rate = if num_opps > 0 {
        win_count as f64 / num_opps as f64
    } else {
        0.0
    };

    Ok(SweepResult {
        combo: combo.clone(),
        num_opps,
        avg_net_bps,
        sharpe,
        max_drawdown_bps: max_dd,
        win_rate,
    })
}
