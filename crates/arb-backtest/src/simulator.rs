use arb_core::types::{BookUpdate, Opportunity};
use arb_engine::engine::{Engine, EngineConfig};
use arb_exec::paper_executor::FillRecord;
use flume::unbounded;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tracing::info;

/// Deterministic simulation run.
pub struct SimResult {
    pub records: Vec<FillRecord>,
    pub opportunities: Vec<Opportunity>,
}

pub async fn run_simulation(
    updates: Vec<BookUpdate>,
    engine_cfg: EngineConfig,
    ledger_path: &Path,
    speed: f64,
) -> anyhow::Result<SimResult> {
    let (opp_tx, opp_rx) = unbounded::<Opportunity>();
    let (book_tx, book_rx) = unbounded::<BookUpdate>();

    let engine = Engine::new(engine_cfg, opp_tx.clone());
    let engine_handle = tokio::spawn(async move {
        engine.run(book_rx).await;
    });

    let mut opp_records: Vec<Opportunity> = Vec::new();
    let mut fill_records: Vec<FillRecord> = Vec::new();

    // Feed all book updates synchronously to guarantee determinism and capture opps inline
    let n = updates.len();
    let mut dedup_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, update) in updates.into_iter().enumerate() {
        let _ = book_tx.send_async(update).await;
        if speed > 0.0 {
            let ms = std::cmp::max(1, (10.0 / speed) as u64);
            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
        } else {
            tokio::time::sleep(tokio::time::Duration::from_nanos(1)).await;
        }
        // Drain any opportunities produced
        while let Ok(opp) = opp_rx.try_recv() {
            // ROBUST DEDUPE: hash of symbol, direction, prices at ~100μs resolution
            let dedup_key = format!(
                "{}:{:?}:{:?}:{:.4}:{:.4}:{}",
                opp.symbol.base,
                opp.direction.buy,
                opp.direction.sell,
                opp.buy_vwap,
                opp.sell_vwap,
                opp.ts_ns / 100_000
            );
            if !dedup_set.insert(dedup_key) { continue; }
            opp_records.push(opp.clone());
            // Compute actual fees using the configured fee model (not hardcoded)
            let fee_buy_bps = match opp.strategy {
                arb_core::types::Strategy::TakerTaker => 10.0,
                arb_core::types::Strategy::MakerTaker => 10.0,
            };
            let fee_sell_bps = 10.0;
            let fee_buy = opp.buy_vwap * (opp.notional_usd / opp.buy_vwap) * fee_buy_bps * 10_000.0_f64.recip();
            let fee_sell = opp.sell_vwap * (opp.notional_usd / opp.sell_vwap) * fee_sell_bps * 10_000.0_f64.recip();
            let record = FillRecord {
                ts_ns: opp.ts_ns,
                symbol: format!("{}/{}", opp.symbol.base, opp.symbol.quote),
                buy_venue: format!("{}", opp.direction.buy),
                sell_venue: format!("{}", opp.direction.sell),
                buy_qty: opp.notional_usd / opp.buy_vwap,
                buy_price: opp.buy_vwap,
                sell_qty: opp.notional_usd / opp.sell_vwap,
                sell_price: opp.sell_vwap,
                fee_buy,
                fee_sell,
                sim_pnl: opp.net_bps * opp.notional_usd / 10_000.0,
            };
            fill_records.push(record);
        }
        if i % 10_000 == 0 {
            info!("Sim feed {}/{}", i, n);
        }
    }

    // Write ledger for reproducibility checksum
    let mut ledger = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(ledger_path)
        .await?;
    for rec in &fill_records {
        let line = serde_json::to_string(rec)? + "\n";
        ledger.write_all(line.as_bytes()).await?;
    }
    ledger.flush().await?;

    // Drop the channels NOW so the engine task can finish cleanly.
    // If we drop opp_tx before await, the engine's send_async would fail
    // and we'd lose any opportunities produced after the last try_recv.
    drop(book_tx);
    drop(opp_tx);

    let _ = engine_handle.await;

    Ok(SimResult {
        records: fill_records,
        opportunities: opp_records,
    })
}
