use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Trading venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Venue {
    Binance,
    Kraken,
    Okx,
}

impl Venue {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Binance => "binance",
            Self::Kraken => "kraken",
            Self::Okx => "okx",
        }
    }
}

impl fmt::Display for Venue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Normalized base/quote pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NormalizedSymbol {
    pub base: String,
    pub quote: String,
}

impl NormalizedSymbol {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }
}

impl fmt::Display for NormalizedSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// Build mode — Live variant only exists when `live` feature is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildMode {
    Paper,
    Observe,
    Test,
    Live,
}

/// Direction of arbitrage (buy venue, sell venue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Direction {
    pub buy: Venue,
    pub sell: Venue,
}

impl Direction {
    pub fn new(buy: Venue, sell: Venue) -> Self {
        Self { buy, sell }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "buy_{}_sell_{}", self.buy.as_str(), self.sell.as_str())
    }
}

/// Execution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    TakerTaker,
    MakerTaker,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Strategy::TakerTaker => write!(f, "taker_taker"),
            Strategy::MakerTaker => write!(f, "maker_taker"),
        }
    }
}

/// Normalized L2 book update.
#[derive(Debug, Clone)]
pub struct BookUpdate {
    pub venue: Venue,
    pub symbol: NormalizedSymbol,
    pub ts_ns: u64,
    pub seq: u64,
    pub bids: Vec<(ordered_float::OrderedFloat<f64>, f64)>,
    pub asks: Vec<(ordered_float::OrderedFloat<f64>, f64)>,
    pub is_snapshot: bool,
}

/// Detected arbitrage opportunity.
#[derive(Debug, Clone, Serialize)]
pub struct Opportunity {
    pub ts_ns: u64,
    pub symbol: NormalizedSymbol,
    pub direction: Direction,
    pub strategy: Strategy,
    pub buy_vwap: f64,
    pub sell_vwap: f64,
    pub notional_usd: f64,
    pub gross_bps: f64,
    pub net_bps: f64,
    pub slippage_ewma_bps: f64,
    pub book_age_ms: HashMap<Venue, f64>,
}

/// Balances per asset.
#[derive(Debug, Clone, Default)]
pub struct Balances {
    pub inner: std::collections::HashMap<String, f64>,
}

impl Balances {
    pub fn get(&self, asset: &str) -> f64 {
        self.inner.get(asset).copied().unwrap_or(0.0)
    }
}

/// Exchange adapter trait.
#[async_trait::async_trait]
pub trait ExchangeAdapter: Send + Sync {
    async fn start(&self, tx: flume::Sender<BookUpdate>) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
    async fn subscribe(&self, symbols: &[NormalizedSymbol]) -> anyhow::Result<()>;
    async fn get_balances(&self) -> anyhow::Result<Balances>;
    fn venue(&self) -> Venue;
}

/// Symbol triple used to map a single logical symbol across venues.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SymbolTriple {
    pub binance: Option<String>,
    pub kraken: Option<String>,
    pub okx: Option<String>,
    pub base: String,
    pub quote: String,
}

impl SymbolTriple {
    pub fn venue_symbol(&self, venue: Venue) -> Option<&String> {
        match venue {
            Venue::Binance => self.binance.as_ref(),
            Venue::Kraken => self.kraken.as_ref(),
            Venue::Okx => self.okx.as_ref(),
        }
    }

    pub fn normalized(&self) -> NormalizedSymbol {
        NormalizedSymbol::new(self.base.clone(), self.quote.clone())
    }
}
