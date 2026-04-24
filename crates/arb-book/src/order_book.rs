use arb_core::types::Venue;
use ordered_float::OrderedFloat;
use std::collections::BTreeMap;
use std::time::Instant;

/// Single-side depth (price -> qty).
pub type Depth = BTreeMap<OrderedFloat<f64>, f64>;

/// VWAP result.
#[derive(Debug, Clone)]
pub struct VwapResult {
    pub vwap: f64,
    pub notional: f64,
    pub qty: f64,
    pub levels_touched: usize,
}

/// Normalized venue-specific order book.
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub venue: Venue,
    pub symbol: String,
    pub bids: Depth,
    pub asks: Depth,
    pub last_seq: u64,
    pub last_update_ts: Instant,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self {
            venue: Venue::Binance,
            symbol: String::new(),
            bids: Depth::new(),
            asks: Depth::new(),
            last_seq: 0,
            last_update_ts: Instant::now(),
        }
    }
}

impl OrderBook {
    /// Build from a REST snapshot.
    pub fn apply_snapshot(
        &mut self,
        bids: Vec<(OrderedFloat<f64>, f64)>,
        asks: Vec<(OrderedFloat<f64>, f64)>,
        seq: u64,
    ) {
        self.bids.clear();
        self.asks.clear();
        for (p, q) in bids {
            if q > 0.0 {
                self.bids.insert(p, q);
            }
        }
        for (p, q) in asks {
            if q > 0.0 {
                self.asks.insert(p, q);
            }
        }
        self.last_seq = seq;
        self.last_update_ts = Instant::now();
    }

    /// Apply WS diff.
    pub fn apply_diff(
        &mut self,
        bids: Vec<(OrderedFloat<f64>, f64)>,
        asks: Vec<(OrderedFloat<f64>, f64)>,
        seq: u64,
    ) {
        for (p, q) in bids {
            if q <= 0.0 {
                self.bids.remove(&p);
            } else {
                self.bids.insert(p, q);
            }
        }
        for (p, q) in asks {
            if q <= 0.0 {
                self.asks.remove(&p);
            } else {
                self.asks.insert(p, q);
            }
        }
        self.last_seq = seq;
        self.last_update_ts = Instant::now();
    }

    /// VWAP over a side for `target_notional` USD.
    pub fn vwap(&self, side: Side, target_notional: f64) -> Option<VwapResult> {
        let iterator: Box<dyn Iterator<Item = (&OrderedFloat<f64>, &f64)> + '_> = match side {
            Side::Bid => {
                // Walk bids descending (best price first)
                Box::new(self.bids.iter().rev())
            }
            Side::Ask => {
                // Walk asks ascending
                Box::new(self.asks.iter())
            }
        };

        let mut notional = 0.0;
        let mut qty = 0.0;
        let mut levels_touched = 0usize;

        for (&price, &q) in iterator {
            let price_f = price.into_inner();
            let level_notional = price_f * q;
            let needed = target_notional - notional;

            if level_notional >= needed {
                let fill_qty = needed / price_f;
                qty += fill_qty;
                notional += price_f * fill_qty;
                levels_touched += 1;
                break;
            } else {
                qty += q;
                notional += level_notional;
                levels_touched += 1;
            }
        }

        if qty > 0.0 {
            Some(VwapResult {
                vwap: notional / qty,
                notional,
                qty,
                levels_touched,
            })
        } else {
            None
        }
    }

    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        self.last_update_ts.elapsed() > max_age
    }

    /// Best bid (price, qty).
    pub fn best_bid(&self) -> Option<(f64, f64)> {
        self.bids
            .iter()
            .rev()
            .next()
            .map(|(p, q)| (p.into_inner(), *q))
    }

    /// Best ask (price, qty).
    pub fn best_ask(&self) -> Option<(f64, f64)> {
        self.asks.iter().next().map(|(p, q)| (p.into_inner(), *q))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy)]
pub enum CrossSide {
    Buy,
    Sell,
}
