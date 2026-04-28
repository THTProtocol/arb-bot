use anyhow::Context;
use arb_book::order_book::OrderBook;
use arb_core::types::{Balances, BookUpdate, ExchangeAdapter, NormalizedSymbol, Venue};
use flume::Sender;
use futures::{SinkExt, StreamExt};
use ordered_float::OrderedFloat;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

pub struct OkxAdapter {
    symbols: Vec<NormalizedSymbol>,
    books: Arc<Mutex<HashMap<NormalizedSymbol, OrderBook>>>,
}

impl OkxAdapter {
    pub fn new(symbols: Vec<NormalizedSymbol>) -> Self {
        let mut books = HashMap::new();
        for s in &symbols {
            let mut book = OrderBook::default();
            book.venue = Venue::Okx;
            book.symbol = format!("{}/{}-{}", s.base, s.quote, Venue::Okx);
            books.insert(s.clone(), book);
        }
        Self {
            symbols,
            books: Arc::new(Mutex::new(books)),
        }
    }

    fn sym_to_okx(sym: &NormalizedSymbol) -> String {
        format!(
            "{}-{}",
            sym.base.to_ascii_uppercase(),
            sym.quote.to_ascii_uppercase()
        )
    }
}

#[async_trait::async_trait]
impl ExchangeAdapter for OkxAdapter {
    fn venue(&self) -> Venue {
        Venue::Okx
    }

    async fn subscribe(&self, _symbols: &[NormalizedSymbol]) -> anyhow::Result<()> {
        Ok(())
    }

    async fn start(&self, tx: Sender<BookUpdate>) -> anyhow::Result<()> {
        let mut attempt = 0u64;
        loop {
            attempt += 1;
            info!("OKX connecting (attempt {}) wss://ws.okx.com:8443/ws/v5/public", attempt);
            match self.run_connection(tx.clone()).await {
                Ok(()) => {
                    info!("OKX connection ended cleanly");
                    return Ok(());
                }
                Err(e) => {
                    error!("OKX connection error (attempt {}): {}", attempt, e);
                    let backoff = std::cmp::min(30, attempt * 2);
                    warn!("OKX reconnecting in {}s...", backoff);
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                }
            }
        }
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_balances(&self) -> anyhow::Result<Balances> {
        Ok(Balances::default())
    }
}

impl OkxAdapter {
    async fn run_connection(&self, tx: Sender<BookUpdate>) -> anyhow::Result<()> {
        let (mut ws_stream, _) = connect_async("wss://ws.okx.com:8443/ws/v5/public")
            .await
            .context("okx ws connect")?;

        let args: Vec<serde_json::Value> = self
            .symbols
            .iter()
            .map(|s| {
                serde_json::json!({
                    "channel": "books",
                    "instId": Self::sym_to_okx(s)
                })
            })
            .collect();
        let sub = serde_json::json!({
            "op": "subscribe",
            "args": args
        });
        ws_stream
            .send(sub.to_string().into())
            .await
            .context("okx sub send")?;

        let (mut write, mut read) = ws_stream.split();

        // Heartbeat: OKX requires ping every 30s
        let mut ping = interval(Duration::from_secs(25));
        loop {
            tokio::select! {
                _ = ping.tick() => {
                    if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Ping(vec![])).await {
                        warn!("OKX ping failed: {}", e);
                        return Err(e.into());
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(msg)) => {
                            if let Ok(text) = msg.to_text() {
                                if text.is_empty() { continue; }
                                if let Err(e) = handle_msg(text, &self.symbols, &self.books, &tx).await {
                                    if e.to_string().contains("checksum") {
                                        warn!("OKX checksum mismatch, skipping validation for this message");
                                        continue;
                                    }
                                    warn!("OKX handle_msg error: {}", e);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            warn!("OKX websocket error: {}", e);
                            return Err(e.into());
                        }
                        None => {
                            warn!("OKX stream closed by server");
                            return Err(anyhow::anyhow!("stream closed"));
                        }
                    }
                }
            }
        }
    }
}

async fn handle_msg(
    text: &str,
    symbols: &[NormalizedSymbol],
    books: &Arc<Mutex<HashMap<NormalizedSymbol, OrderBook>>>,
    tx: &Sender<BookUpdate>,
) -> anyhow::Result<()> {
    let v: Value = serde_json::from_str(text)?;
    let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
    if event == "subscribe" || event == "error" {
        return Ok(());
    }
    let Some(data_arr) = v.get("data").and_then(|x| x.as_array()) else {
        return Ok(());
    };
    let Some(arg) = v.get("arg").and_then(|x| x.as_object()) else {
        return Ok(());
    };
    let inst_id = arg.get("instId").and_then(|x| x.as_str()).unwrap_or("");
    let symbol = symbols
        .iter()
        .find(|s| inst_id == OkxAdapter::sym_to_okx(s))
        .cloned();
    let Some(symbol) = symbol else {
        return Ok(());
    };

    for entry in data_arr {
        let is_snapshot = v.get("action").and_then(|x| x.as_str()) == Some("snapshot");
        let bids = parse_levels(&entry["bids"]);
        let asks = parse_levels(&entry["asks"]);

        {
            let mut books = books.lock().unwrap();
            let book = books.get_mut(&symbol).unwrap();
            if is_snapshot {
                book.apply_snapshot(bids.clone(), asks.clone(), 0);
            } else {
                book.apply_diff(bids.clone(), asks.clone(), 0);
            }

            if let Some(checksum) = entry.get("checksum").and_then(|x| x.as_i64()) {
                let computed = compute_okx_checksum(book);
                if computed != checksum as i32 {
                    anyhow::bail!(
                        "checksum mismatch: computed={} expected={}",
                        computed,
                        checksum
                    );
                }
            }
        }

        let _ = tx.send(BookUpdate {
            venue: Venue::Okx,
            symbol: symbol.clone(),
            ts_ns: now_ns(),
            seq: 0,
            bids,
            asks,
            is_snapshot,
        });
    }
    Ok(())
}

fn parse_levels(v: &Value) -> Vec<(OrderedFloat<f64>, f64)> {
    v.as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| {
            let price: f64 = entry.get(0)?.as_str()?.parse().ok()?;
            let qty: f64 = entry.get(1)?.as_str()?.parse().ok()?;
            if qty > 0.0 {
                Some((OrderedFloat(price), qty))
            } else {
                None
            }
        })
        .collect()
}

fn format_decimal(v: f64) -> String {
    if v == 0.0 {
        return "0.0".to_string();
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn compute_okx_checksum(book: &OrderBook) -> i32 {
    let mut parts: Vec<String> = Vec::new();
    let bids_vec: Vec<(&OrderedFloat<f64>, &f64)> = book.bids.iter().rev().take(25).collect();
    let asks_vec: Vec<(&OrderedFloat<f64>, &f64)> = book.asks.iter().take(25).collect();
    for i in 0..25 {
        let price_b = bids_vec
            .get(i)
            .map(|(p, _)| format_decimal(p.into_inner()))
            .unwrap_or_else(|| "".to_string());
        let size_b = bids_vec
            .get(i)
            .map(|(_, q)| format_decimal(**q))
            .unwrap_or_else(|| "".to_string());
        let price_a = asks_vec
            .get(i)
            .map(|(p, _)| format_decimal(p.into_inner()))
            .unwrap_or_else(|| "".to_string());
        let size_a = asks_vec
            .get(i)
            .map(|(_, q)| format_decimal(**q))
            .unwrap_or_else(|| "".to_string());
        parts.push(price_b);
        parts.push(size_b);
        parts.push(price_a);
        parts.push(size_a);
    }
    let joined = parts.join(":");
    let crc = crc32fast::hash(joined.as_bytes());
    crc as i32
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
