use arb_adapters::{BinanceAdapter, KrakenAdapter, OkxAdapter};
use arb_config::loader::load;
use arb_core::types::{BookUpdate, ExchangeAdapter, Venue};
use anyhow::Context;
use chrono::Timelike;
use clap::Parser;
use flume::bounded;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

#[derive(Debug, Parser)]
#[command(name = "arb-recorder")]
struct Args {
    #[arg(long, default_value = "24")]
    hours: u64,
    #[arg(long, default_value = "recordings")]
    out: String,
    #[arg(long, value_delimiter = ',', default_value = "binance,kraken,okx")]
    venues: Vec<String>,
    #[arg(long, default_value = "config.yaml")]
    config: String,
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
        .init();

    let args = Args::parse();
    let cfg = load(Path::new(&args.config)).context("load config")?;

    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = Path::new(&args.out).join(&day);
    tokio::fs::create_dir_all(&out_dir).await?;

    let (tx, rx) = bounded::<BookUpdate>(10_000);
    let mut files: std::collections::HashMap<String, tokio::fs::File> = Default::default();
    let mut last_hour = u32::MAX;

    let mut handles = Vec::new();
    for venue_str in &args.venues {
        let venue = venue_str.to_lowercase();
        let tx = tx.clone();
        let syms = cfg
            .symbols
            .iter()
            .filter(|s| {
                match venue.as_str() {
                    "binance" => s.binance.is_some(),
                    "kraken"  => s.kraken.is_some(),
                    "okx"     => s.okx.is_some(),
                    _ => false,
                }
            })
            .map(|s| {
                match venue.as_str() {
                    "binance" => arb_core::types::NormalizedSymbol::new(
                        s.binance.as_ref().unwrap()[..s.binance.as_ref().unwrap().len()-4].to_string(),
                        s.binance.as_ref().unwrap()[s.binance.as_ref().unwrap().len()-4..].to_string().to_uppercase(),
                    ),
                    "kraken" => {
                        let parts: Vec<&str> = s.kraken.as_ref().unwrap().split('/').collect();
                        arb_core::types::NormalizedSymbol::new(
                            parts.get(0).unwrap_or(&"").to_string().replace("XBT","BTC"),
                            parts.get(1).unwrap_or(&"").to_string(),
                        )
                    }
                    "okx" => {
                        let parts: Vec<&str> = s.okx.as_ref().unwrap().split('-').collect();
                        arb_core::types::NormalizedSymbol::new(
                            parts.get(0).unwrap_or(&"").to_string(),
                            parts.get(1).unwrap_or(&"").to_string(),
                        )
                    }
                    _ => arb_core::types::NormalizedSymbol::new(
                        String::from("BTC"),
                        String::from("USDT"),
                    ),
                }
            })
            .collect::<Vec<_>>();

        let handle: tokio::task::JoinHandle<anyhow::Result<()>> = match venue.as_str() {
            "binance" => {
                let a = BinanceAdapter::new(syms);
                tokio::spawn(async move { a.start(tx).await })
            }
            "kraken" => {
                let a = KrakenAdapter::new(syms);
                tokio::spawn(async move { a.start(tx).await })
            }
            "okx" => {
                let a = OkxAdapter::new(syms);
                tokio::spawn(async move { a.start(tx).await })
            }
            other => {
                warn!("Unknown venue {}", other);
                continue;
            }
        };
        handles.push((venue, handle));
    }

    // Spawn handle monitor so adapter deaths are logged immediately
    let monitor_handle = tokio::spawn(async move {
        for (venue, h) in handles {
            match h.await {
                Ok(Ok(())) => info!("{} adapter exited cleanly", venue),
                Ok(Err(e)) => error!("{} adapter failed: {}", venue, e),
                Err(e) => error!("{} adapter panicked: {}", venue, e),
            }
        }
    });

    let limit = chrono::Duration::hours(args.hours as i64);
    let start = chrono::Utc::now();
    let mut total = 0usize;

    while chrono::Utc::now() - start < limit {
        let hour = chrono::Utc::now().hour();
        if hour != last_hour {
            if last_hour != u32::MAX {
                for (key, _f) in files.drain() {
                    let gz = format!("{}.gz", &key);
                    tokio::task::spawn_blocking({
                        let k = key.clone();
                        let gz = gz.clone();
                        move || {
                            if let Ok(meta) = std::fs::metadata(&k) {
                                if meta.len() == 0 { return; }
                            }
                            if let Ok(f) = std::fs::File::open(&k) {
                                let mut r = std::io::BufReader::new(f);
                                let w = std::fs::File::create(&gz);
                                if let Ok(w) = w {
                                    let mut enc = flate2::write::GzEncoder::new(w, flate2::Compression::default());
                                    let _ = std::io::copy(&mut r, &mut enc);
                                    let _ = enc.finish();
                                    let _ = std::fs::remove_file(&k);
                                }
                            }
                        }
                    });
                }
            }
            last_hour = hour;
        }

        if let Ok(upd) = rx.recv_async().await {
            total += 1;
            let key = format!(
                "{}/{}_{}_{}.jsonl",
                out_dir.display(),
                upd.venue,
                upd.symbol.base,
                upd.symbol.quote
            );
            if !files.contains_key(&key) {
                let f = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&key)
                    .await?;
                files.insert(key.clone(), f);
            }
            let line = serde_json::json!({
                "venue": format!("{}", upd.venue),
                "symbol": format!("{}/{}", upd.symbol.base, upd.symbol.quote),
                "ts_ns": upd.ts_ns,
                "seq": upd.seq,
                "bids": upd.bids.iter().map(|(p,q)| [p.into_inner(), *q]).collect::<Vec<_>>(),
                "asks": upd.asks.iter().map(|(p,q)| [p.into_inner(), *q]).collect::<Vec<_>>(),
                "is_snapshot": upd.is_snapshot,
            });
            let buf = serde_json::to_string(&line)? + "\n";
            if let Some(f) = files.get_mut(&key) {
                f.write_all(buf.as_bytes()).await?;
            }
        }
        if total % 10_000 == 0 {
            info!("Recorded {} updates", total);
        }
    }

    info!("Recording complete: {} updates", total);
    Ok(())
}
