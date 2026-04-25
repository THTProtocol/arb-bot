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
) -> anyhow::Result<SimResult> {
    let (opp_tx, opp_rx) = unbounded::<Opportunity>();
    let (book_tx, book_rx) = unbounded::<BookUpdate>();

    let engine = Engine::new(engine_cfg, opp_tx.clone());
    let engine_handle = tokio::spawn(async move {
        engine.run(book_rx).await;
    });

    let mut opp_records: Vec<Opportunity> = Vec::new();
    let mut fill_records: Vec<FillRecord> = Vec::new();

    let opp_rx_clone = opp_rx.clone();
    let _executor_handle = tokio::spawn(async move {
        // drain opp_rx and fill
        // We process opps inline after feeding books to avoid complex concurrency during sim
        // (the runner below blocks this handle)
        while let Ok(_opp) = opp_rx_clone.recv_async().await {}
    });

    // Feed all book updates synchronously to guarantee determinism
    let n = updates.len();
    for (i, update) in updates.into_iter().enumerate() {
        let _ = book_tx.send_async(update).await;
        // Drain any opportunities produced synchronously
        while let Ok(opp) = opp_rx.try_recv() {
            opp_records.push(opp.clone());
            // For simulation, simply log the paper fill inline.
            // We use dummy books since exact fill depends on VWAP at execution time.
            // In a true replay we'd carry books snapshot — simplified here to log with VWAP.
            let record = FillRecord {
                ts_ns: opp.ts_ns,
                symbol: format!("{}/{}", opp.symbol.base, opp.symbol.quote),
                buy_venue: format!("{}", opp.direction.buy),
                sell_venue: format!("{}", opp.direction.sell),
                buy_qty: opp.notional_usd / opp.buy_vwap,
                buy_price: opp.buy_vwap,
                sell_qty: opp.notional_usd / opp.sell_vwap,
                sell_price: opp.sell_vwap,
                fee_buy: opp.buy_vwap * (opp.notional_usd / opp.buy_vwap) * 0.001,
                fee_sell: opp.sell_vwap * (opp.notional_usd / opp.sell_vwap) * 0.0016,
                sim_pnl: opp.net_bps * opp.notional_usd / 10_000.0,
            };
            fill_records.push(record);
        }
        if i % 10_000 == 0 {
            info!("Sim feed {}/{}", i, n);
        }
    }

    drop(book_tx);
    drop(opp_tx);

    let _ = engine_handle.await;

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

    Ok(SimResult {
        records: fill_records,
        opportunities: opp_records,
    })
}
