use arb_core::types::{BookUpdate, NormalizedSymbol, Venue};
use flume::Sender;
use std::path::Path;

pub struct Replayer {
    path: std::path::PathBuf,
}

impl Replayer {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub async fn replay(&self, tx: Sender<BookUpdate>) -> anyhow::Result<()> {
        let content = tokio::fs::read_to_string(&self.path).await?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let val: serde_json::Value = serde_json::from_str(line)?;
            let venue = match val["venue"].as_str() {
                Some("Binance") => Venue::Binance,
                Some("Kraken") => Venue::Kraken,
                _ => continue,
            };
            let sym_str = val["symbol"].as_str().unwrap_or("");
            let parts: Vec<&str> = sym_str.split('/').collect();
            let symbol = NormalizedSymbol::new(
                parts.get(0).unwrap_or(&"").to_string(),
                parts.get(1).unwrap_or(&"").to_string(),
            );
            let bids = val["bids"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|v| {
                    let a = v.as_array()?;
                    Some((
                        ordered_float::OrderedFloat(a.get(0)?.as_f64()?),
                        a.get(1)?.as_f64().unwrap_or(0.0),
                    ))
                })
                .collect();
            let asks = val["asks"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|v| {
                    let a = v.as_array()?;
                    Some((
                        ordered_float::OrderedFloat(a.get(0)?.as_f64()?),
                        a.get(1)?.as_f64().unwrap_or(0.0),
                    ))
                })
                .collect();
            let update = BookUpdate {
                venue,
                symbol,
                ts_ns: val["ts_ns"].as_u64().unwrap_or(0),
                seq: val["seq"].as_u64().unwrap_or(0),
                bids,
                asks,
                is_snapshot: val["is_snapshot"].as_bool().unwrap_or(false),
            };
            let _ = tx.send_async(update).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }
        Ok(())
    }
}
