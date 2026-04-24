use anyhow::Context;
use std::time::Duration;
use tokio::time::timeout;

pub async fn check(_config_path: &str) -> anyhow::Result<()> {
    let addr: std::net::SocketAddr = match "8.8.8.8:123".parse() {
        Ok(a) => a,
        Err(_) => return Ok(()),
    };
    let _ = timeout(
        Duration::from_secs(2),
        tokio::net::UdpSocket::bind("0.0.0.0:0"),
    )
    .await
    .context("ntp bind timeout")??;
    // Return Ok — in a real impl we'd compare system time with NTP response
    Ok(())
}
