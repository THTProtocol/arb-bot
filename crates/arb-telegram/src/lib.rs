use serde::Serialize;
use tracing::{error, info};

pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
struct TelegramMessage {
    chat_id: String,
    text: String,
    parse_mode: String,
}

impl TelegramNotifier {
    pub fn new(bot_token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn send(&self,
        text: impl Into<String>,
    ) {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
        let payload = TelegramMessage {
            chat_id: self.chat_id.clone(),
            text: text.into(),
            parse_mode: "HTML".into(),
        };
        match self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    error!("Telegram API error: {}", body);
                } else {
                    info!("Telegram notification sent");
                }
            }
            Err(e) => {
                error!("Telegram request failed: {}", e);
            }
        }
    }

    pub async fn online(&self, venues: &[impl AsRef<str>], mode: &str) {
        let v = venues.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(", ");
        let text = format!(
            r#"<b>🤖 Arbitrage Bot ONLINE</b>

Mode: <code>{}</code>
Venues: <code>{}</code>
Max Notional: $1000 USD
"#,
            mode, v
        );
        self.send(text).await;
    }

    pub async fn trade(&self,
        symbol: &str,
        direction: &str,
        pnl: f64,
        buy_px: f64,
        sell_px: f64,
    ) {
        let emoji = if pnl >= 0.0 { "🟢" } else { "🔴" };
        let text = format!(
            r#"{} <b>{}</b>
Direction: {}
Buy @{:.4}
Sell @{:.4}
PnL: {:.4}
"#,
            emoji, symbol, direction, buy_px, sell_px, pnl
        );
        self.send(text).await;
    }
}
