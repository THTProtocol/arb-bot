use arb_core::types::BookUpdate;
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug)]
pub struct Recorder {
    path: std::path::PathBuf,
}

impl Recorder {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub async fn record(&self, update: &BookUpdate) -> anyhow::Result<()> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        // Serialize simplified line
        let line = serde_json::json!({
            "venue": format!("{}", update.venue),
            "symbol": format!("{}/{}", update.symbol.base, update.symbol.quote),
            "ts_ns": update.ts_ns,
            "seq": update.seq,
            "bids": update.bids.iter().map(|(p,q)| [p.into_inner(), *q]).collect::<Vec<_>>(),
            "asks": update.asks.iter().map(|(p,q)| [p.into_inner(), *q]).collect::<Vec<_>>(),
            "is_snapshot": update.is_snapshot,
        });
        let mut buf = serde_json::to_string(&line)?
            + "
";
        file.write_all(buf.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }
}
