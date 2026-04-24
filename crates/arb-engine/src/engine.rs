use crate::slippage_ewma::SlippageEwma;
use arb_book::order_book::{OrderBook, Side};
use arb_core::fee_model::FeeModel;
use arb_core::types::{
    BookUpdate, BuildMode, Direction, NormalizedSymbol, Opportunity, Strategy, Venue,
};
use flume::{Receiver, Sender};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::info;

pub struct EngineConfig {
    pub mode: BuildMode,
    pub profit_threshold_bps: f64,
    pub notional_usd: f64,
    pub usd_usdt_basis_bps: f64,
    pub strategies: Vec<Strategy>,
    pub symbol_maps: Vec<HashMap<Venue, NormalizedSymbol>>,
    pub enabled_venues: Vec<Venue>,
    pub fee_model: FeeModel,
    pub slippage_alpha: f64,
}

pub struct Engine {
    cfg: EngineConfig,
    books: HashMap<(Venue, NormalizedSymbol), (OrderBook, Instant)>,
    ewma: SlippageEwma,
    opp_tx: Sender<Opportunity>,
}

impl Engine {
    pub fn new(cfg: EngineConfig, opp_tx: Sender<Opportunity>) -> Self {
        Self {
            books: HashMap::new(),
            ewma: SlippageEwma::new(cfg.slippage_alpha),
            cfg,
            opp_tx,
        }
    }

    pub async fn run(mut self, rx: Receiver<BookUpdate>) {
        while let Ok(update) = rx.recv_async().await {
            let key = (update.venue, update.symbol.clone());
            let book = self
                .books
                .entry(key)
                .or_insert_with(|| (OrderBook::default(), Instant::now()));
            if update.is_snapshot {
                book.0.apply_snapshot(update.bids, update.asks, update.seq);
            } else {
                book.0.apply_diff(update.bids, update.asks, update.seq);
            }
            book.1 = Instant::now();
            self.evaluate_all().await;
        }
    }

    async fn evaluate_all(&mut self) {
        let maps = self.cfg.symbol_maps.clone();
        let strategies = self.cfg.strategies.clone();
        let threshold = self.cfg.profit_threshold_bps;
        let mode = self.cfg.mode;
        let notional = self.cfg.notional_usd;
        let usd_usdt = self.cfg.usd_usdt_basis_bps;
        let directions = all_directions(&self.cfg.enabled_venues);

        for sym_map in &maps {
            for &(buy_v, sell_v) in &directions {
                let Some(buy_sym) = sym_map.get(&buy_v) else {
                    continue;
                };
                let Some(sell_sym) = sym_map.get(&sell_v) else {
                    continue;
                };
                for strat in &strategies {
                    if let Some(opp) = self.evaluate(
                        buy_v, sell_v, buy_sym, sell_sym, strat, notional, usd_usdt, threshold,
                    ) {
                        if mode == BuildMode::Paper || mode == BuildMode::Observe {
                            let _ = self.opp_tx.send_async(opp).await;
                        }
                    }
                }
            }
        }
    }

    fn evaluate(
        &self,
        buy_v: Venue,
        sell_v: Venue,
        buy_sym: &NormalizedSymbol,
        sell_sym: &NormalizedSymbol,
        strat: &Strategy,
        notional_usd: f64,
        usd_usdt_basis_bps: f64,
        profit_threshold_bps: f64,
    ) -> Option<Opportunity> {
        let buy_book = self.books.get(&(buy_v, buy_sym.clone()))?;
        let sell_book = self.books.get(&(sell_v, sell_sym.clone()))?;
        let buy_vwap = buy_book.0.vwap(Side::Ask, notional_usd)?;
        let sell_vwap = sell_book.0.vwap(Side::Bid, notional_usd)?;

        let fee_buy = match strat {
            Strategy::TakerTaker => self.cfg.fee_model.fee_fraction(buy_v, true),
            Strategy::MakerTaker => self.cfg.fee_model.fee_fraction(buy_v, false),
        };
        let fee_sell = self.cfg.fee_model.fee_fraction(sell_v, true);

        let buy_cost = buy_vwap.vwap * (1.0 + fee_buy);
        let sell_rev = sell_vwap.vwap * (1.0 - fee_sell);
        let gross_bps = ((sell_vwap.vwap - buy_vwap.vwap) / buy_vwap.vwap) * 10_000.0;
        let net_bps = ((sell_rev - buy_cost) / buy_vwap.vwap) * 10_000.0;

        let ewma_key = (
            format!("{}{}", buy_sym.base, buy_sym.quote),
            format!("{}", buy_v),
        );
        let slippage = self.ewma.get(&ewma_key);
        let mut final_net = net_bps - slippage;
        if sell_sym.quote != buy_sym.quote {
            final_net -= usd_usdt_basis_bps;
        }

        if final_net <= profit_threshold_bps {
            return None;
        }

        let mut book_age_ms = HashMap::new();
        for &v in &self.cfg.enabled_venues {
            let age = if let Some(b) = self.books.get(&(v, buy_sym.clone())) {
                b.1.elapsed().as_millis() as f64
            } else {
                f64::MAX
            };
            book_age_ms.insert(v, age);
        }

        Some(Opportunity {
            ts_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64,
            symbol: buy_sym.clone(),
            direction: Direction::new(buy_v, sell_v),
            strategy: *strat,
            buy_vwap: buy_vwap.vwap,
            sell_vwap: sell_vwap.vwap,
            notional_usd: notional_usd.min(buy_vwap.notional).min(sell_vwap.notional),
            gross_bps,
            net_bps: final_net,
            slippage_ewma_bps: slippage,
            book_age_ms,
        })
    }
}

fn all_directions(enabled: &[Venue]) -> Vec<(Venue, Venue)> {
    let mut out = Vec::new();
    for &buy in enabled {
        for &sell in enabled {
            if buy != sell {
                out.push((buy, sell));
            }
        }
    }
    out
}
