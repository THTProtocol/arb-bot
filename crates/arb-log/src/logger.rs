use arb_core::types::Opportunity;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tracing::info;

pub struct OpportunityLogger {
    path: std::path::PathBuf,
}

impl OpportunityLogger {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub async fn log(&self, opp: &Opportunity) -> anyhow::Result<()> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        let line = serde_json::to_string(opp)? + "\n";
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        drop(file);
        info!(
            "Opportunity {} {} net={:.2}bps",
            opp.symbol, opp.direction, opp.net_bps
        );
        Ok(())
    }
}

impl Clone for OpportunityLogger {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
        }
    }
}
