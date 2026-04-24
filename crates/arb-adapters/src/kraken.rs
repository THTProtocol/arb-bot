use anyhow::Context;
use arb_book::order_book::OrderBook;
use arb_core::types::{BookUpdate, ExchangeAdapter, NormalizedSymbol, Venue};
use flume::Sender;
use futures::{SinkExt, StreamExt};
use ordered_float::OrderedFloat;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

pub struct KrakenAdapter {
    symbols: Vec<NormalizedSymbol>,
    books: Arc<Mutex<HashMap<NormalizedSymbol, OrderBook>>>,
}

impl KrakenAdapter {
    pub fn new(symbols: Vec<NormalizedSymbol>) -> Self {
        let mut books = HashMap::new();
        for s in &symbols {
            let mut book = OrderBook::default();
            book.venue = Venue::Kraken;
            book.symbol = format!("{}/{}", s.base, s.quote);
            books.insert(s.clone(), book);
        }
        Self {
            symbols,
            books: Arc::new(Mutex::new(books)),
        }
    }

    fn sym_to_kraken_ws(sym: &NormalizedSymbol) -> String {
        let base = if sym.base.eq_ignore_ascii_case("BTC") {
            "XBT"
        } else {
            &sym.base
        };
        let quote = sym.quote.to_ascii_uppercase();
        format!("{}/{}", base.to_ascii_uppercase(), quote)
    }
}

#[async_trait::async_trait]
impl ExchangeAdapter for KrakenAdapter {
    fn venue(&self) -> Venue {
        Venue::Kraken
    }

    async fn subscribe(&self, _symbols: &[NormalizedSymbol]) -> anyhow::Result<()> {
        Ok(())
    }

    async fn start(&self, tx: Sender<BookUpdate>) -> anyhow::Result<()> {
        info!("Kraken connecting wss://ws.kraken.com");
        let (mut ws_stream, _) = connect_async("wss://ws.kraken.com")
            .await
            .context("kraken ws connect")?;

        let pairs: Vec<String> = self
            .symbols
            .iter()
            .map(|s| Self::sym_to_kraken_ws(s))
            .collect();
        let sub = serde_json::json!({
            "event": "subscribe",
            "pair": pairs,
            "subscription": { "name": "book", "depth": 25 }
        });
        ws_stream
            .send(sub.to_string().into())
            .await
            .context("kraken sub send")?;

        let (_, mut read) = ws_stream.split();
        while let Some(msg) = read.next().await {
            let msg = msg.context("kraken ws msg")?;
            if let Ok(text) = msg.to_text() {
                if let Err(e) = handle_msg(text, &self.symbols, &self.books, &tx).await {
                    warn!("Kraken handle_msg error: {}", e);
                    if e.to_string().contains("checksum") {
                        anyhow::bail!("Checksum mismatch, forcing resync");
                    }
                }
            }
        }
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_balances(&self) -> anyhow::Result<arb_core::types::Balances> {
        Ok(arb_core::types::Balances::default())
    }
}

async fn handle_msg(
    text: &str,
    symbols: &[NormalizedSymbol],
    books: &Arc<Mutex<HashMap<NormalizedSymbol, OrderBook>>>,
    tx: &Sender<BookUpdate>,
) -> anyhow::Result<()> {
    let v: Value = serde_json::from_str(text)?;

    // Array-style kraken messages
    if v.is_array() {
        let arr = v.as_array().unwrap();
        if arr.len() < 2 {
            return Ok(());
        }
        let pair = arr[3].as_str().unwrap_or("");
        let symbol = symbols
            .iter()
            .find(|s| {
                let ws = if s.base.eq_ignore_ascii_case("BTC") {
                    format!("XBT/{}", s.quote.to_ascii_uppercase())
                } else {
                    format!(
                        "{}/{}",
                        s.base.to_ascii_uppercase(),
                        s.quote.to_ascii_uppercase()
                    )
                };
                ws == pair
            })
            .cloned();
        let Some(symbol) = symbol else {
            return Ok(());
        };

        let data = &arr[1];
        let is_snapshot = data.get("bs").is_some() && data.get("as").is_some();

        let bids = if is_snapshot {
            parse_levels_kraken(&data["bs"])
        } else {
            parse_levels_kraken(&data["b"])
        };
        let asks = if is_snapshot {
            parse_levels_kraken(&data["as"])
        } else {
            parse_levels_kraken(&data["a"])
        };

        {
            let mut books = books.lock().unwrap();
            let book = books.get_mut(&symbol).unwrap();
            if is_snapshot {
                book.apply_snapshot(bids.clone(), asks.clone(), 0);
            } else {
                book.apply_diff(bids.clone(), asks.clone(), 0);
            }

            if let Some(checksum) = data
                .get("c")
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse::<u64>().ok())
            {
                let computed = compute_crc32(book);
                if computed != checksum as u32 {
                    anyhow::bail!(
                        "checksum mismatch: computed={} expected={}",
                        computed,
                        checksum
                    );
                }
            }
        }

        let _ = tx.send(BookUpdate {
            venue: Venue::Kraken,
            symbol,
            ts_ns: now_ns(),
            seq: 0,
            bids,
            asks,
            is_snapshot,
        });
        return Ok(());
    }

    if let Some(channel) = v.get("channel").and_then(|x| x.as_str()) {
        if channel != "book" {
            return Ok(());
        }
    } else {
        return Ok(());
    }

    let data = match v.get("data").and_then(|x| x.as_array()) {
        Some(d) => d,
        None => return Ok(()),
    };

    for entry in data {
        let sym_str = entry.get("symbol").and_then(|x| x.as_str()).unwrap_or("");
        let symbol = symbols
            .iter()
            .find(|s| {
                let ws = if s.base.eq_ignore_ascii_case("BTC") {
                    format!("XBT/{}", s.quote.to_ascii_uppercase())
                } else {
                    format!(
                        "{}/{}",
                        s.base.to_ascii_uppercase(),
                        s.quote.to_ascii_uppercase()
                    )
                };
                ws == sym_str
            })
            .cloned();
        let Some(symbol) = symbol else {
            continue;
        };

        let is_snapshot = v.get("type").and_then(|x| x.as_str()) == Some("snapshot");
        let bids = parse_levels_kraken(&entry["bids"]);
        let asks = parse_levels_kraken(&entry["asks"]);

        {
            let mut books = books.lock().unwrap();
            let book = books.get_mut(&symbol).unwrap();
            if is_snapshot {
                book.apply_snapshot(bids.clone(), asks.clone(), 0);
            } else {
                book.apply_diff(bids.clone(), asks.clone(), 0);
            }

            if let Some(checksum) = entry.get("checksum").and_then(|x| x.as_u64()) {
                let computed = compute_crc32(book);
                if computed != checksum as u32 {
                    anyhow::bail!(
                        "checksum mismatch: computed={} expected={}",
                        computed,
                        checksum
                    );
                }
            }
        }

        let _ = tx.send(BookUpdate {
            venue: Venue::Kraken,
            symbol,
            ts_ns: now_ns(),
            seq: 0,
            bids,
            asks,
            is_snapshot,
        });
    }
    Ok(())
}

fn parse_levels_kraken(v: &Value) -> Vec<(OrderedFloat<f64>, f64)> {
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

fn compute_crc32(book: &OrderBook) -> u32 {
    let mut buf = String::new();
    for (p, q) in book.asks.iter().take(10) {
        buf.push_str(&format_decimal(p.into_inner()));
        buf.push_str(&format_decimal(*q));
    }
    for (p, q) in book.bids.iter().rev().take(10) {
        buf.push_str(&format_decimal(p.into_inner()));
        buf.push_str(&format_decimal(*q));
    }
    crc32fast::hash(buf.as_bytes())
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
