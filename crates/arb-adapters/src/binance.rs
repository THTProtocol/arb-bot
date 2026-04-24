use anyhow::Context;
use arb_book::order_book::OrderBook;
use arb_core::types::{BookUpdate, ExchangeAdapter, NormalizedSymbol, Venue};
use flume::Sender;
use futures::{SinkExt, StreamExt};
use ordered_float::OrderedFloat;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tracing::{info, warn};

pub struct BinanceAdapter {
    symbols: Vec<NormalizedSymbol>,
    books: Arc<Mutex<HashMap<NormalizedSymbol, OrderBook>>>,
}

impl BinanceAdapter {
    pub fn new(symbols: Vec<NormalizedSymbol>) -> Self {
        let mut books = HashMap::new();
        for s in &symbols {
            let mut book = OrderBook::default();
            book.venue = Venue::Binance;
            book.symbol = format!("{}/{}", s.base, s.quote);
            books.insert(s.clone(), book);
        }
        Self {
            symbols,
            books: Arc::new(Mutex::new(books)),
        }
    }

    fn symbol_to_binance(sym: &NormalizedSymbol) -> String {
        format!(
            "{}{}",
            sym.base.to_ascii_lowercase(),
            sym.quote.to_ascii_lowercase()
        )
    }
}

#[async_trait::async_trait]
impl ExchangeAdapter for BinanceAdapter {
    fn venue(&self) -> Venue {
        Venue::Binance
    }

    async fn subscribe(&self, _symbols: &[NormalizedSymbol]) -> anyhow::Result<()> {
        Ok(())
    }

    async fn start(&self, tx: Sender<BookUpdate>) -> anyhow::Result<()> {
        let streams = self
            .symbols
            .iter()
            .map(|s| format!("{}@depth", Self::symbol_to_binance(s)))
            .collect::<Vec<_>>()
            .join("/");
        let url = format!("wss://stream.binance.com:9443/ws/{}", streams);
        info!("Binance connecting {}", url);
        let (ws_stream, _) = connect_async(&url).await.context("binance ws connect")?;
        let (_, mut rx) = ws_stream.split();

        while let Some(msg) = rx.next().await {
            let msg = msg.context("binance ws msg")?;
            if let Ok(text) = msg.to_text() {
                if let Err(e) = handle_msg(text, &self.symbols, &self.books, &tx).await {
                    warn!("Binance handle_msg error: {}", e);
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
    _books: &Arc<Mutex<HashMap<NormalizedSymbol, OrderBook>>>,
    tx: &Sender<BookUpdate>,
) -> anyhow::Result<()> {
    let v: Value = serde_json::from_str(text)?;
    let event = v.get("e").and_then(|x| x.as_str()).unwrap_or("");
    if event != "depthUpdate" {
        return Ok(());
    }
    let raw_sym = v.get("s").and_then(|x| x.as_str()).unwrap_or("");
    let symbol = symbols
        .iter()
        .find(|s| raw_sym.eq_ignore_ascii_case(&format!("{}{}", s.base, s.quote)))
        .cloned();
    let Some(symbol) = symbol else {
        return Ok(());
    };

    let bids = parse_levels(&v["b"]);
    let asks = parse_levels(&v["a"]);
    let seq = v.get("u").and_then(|x| x.as_u64()).unwrap_or(0);
    let ts = v.get("E").and_then(|x| x.as_u64()).unwrap_or(0);

    let _ = tx.send(BookUpdate {
        venue: Venue::Binance,
        symbol: symbol.clone(),
        ts_ns: ts * 1_000_000,
        seq,
        bids: bids.clone(),
        asks: asks.clone(),
        is_snapshot: false,
    });
    Ok(())
}

fn parse_levels(v: &Value) -> Vec<(OrderedFloat<f64>, f64)> {
    v.as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| {
            let arr = entry.as_array()?;
            let price: f64 = arr.get(0)?.as_str()?.parse().ok()?;
            let qty: f64 = arr.get(1)?.as_str()?.parse().ok()?;
            Some((OrderedFloat(price), qty))
        })
        .collect()
}
