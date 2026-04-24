use crate::inventory::Inventory;
use arb_book::order_book::{OrderBook, Side};
use arb_core::types::{Direction, Opportunity};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::info;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FillRecord {
    pub ts_ns: u64,
    pub symbol: String,
    pub buy_venue: String,
    pub sell_venue: String,
    pub buy_qty: f64,
    pub buy_price: f64,
    pub sell_qty: f64,
    pub sell_price: f64,
    pub fee_buy: f64,
    pub fee_sell: f64,
    pub sim_pnl: f64,
}

#[derive(Debug)]
pub struct PaperExecutor {
    pub ledger_path: PathBuf,
    pub inventory: std::collections::HashMap<String, Inventory>,
}

impl PaperExecutor {
    pub fn new(ledger_path: impl Into<PathBuf>) -> Self {
        Self {
            ledger_path: ledger_path.into(),
            inventory: std::collections::HashMap::new(),
        }
    }

    pub async fn simulate(
        &mut self,
        opp: &Opportunity,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
        _buy_taker: bool,
        _sell_taker: bool,
    ) -> anyhow::Result<FillRecord> {
        let buy_fill = walk(buy_book, Side::Ask, opp.notional_usd);
        let sell_fill = walk(sell_book, Side::Bid, opp.notional_usd);
        let fee_buy = opp.buy_vwap * buy_fill.qty * 0.001;
        let fee_sell = opp.sell_vwap * sell_fill.qty * 0.0016;
        let cost = buy_fill.notional + fee_buy;
        let revenue = sell_fill.notional - fee_sell;
        let pnl = revenue - cost;

        let buy_inv = self
            .inventory
            .entry(format!("{}", opp.direction.buy))
            .or_default();
        buy_inv.update(&opp.symbol.base, buy_fill.qty);
        let sell_inv = self
            .inventory
            .entry(format!("{}", opp.direction.sell))
            .or_default();
        sell_inv.update(&opp.symbol.base, -sell_fill.qty);

        let rec = FillRecord {
            ts_ns: opp.ts_ns,
            symbol: format!("{}/{}", opp.symbol.base, opp.symbol.quote),
            buy_venue: format!("{}", opp.direction.buy),
            sell_venue: format!("{}", opp.direction.sell),
            buy_qty: buy_fill.qty,
            buy_price: opp.buy_vwap,
            sell_qty: sell_fill.qty,
            sell_price: opp.sell_vwap,
            fee_buy,
            fee_sell,
            sim_pnl: pnl,
        };
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ledger_path)
            .await?;
        let line = serde_json::to_string(&rec)? + "\n";
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        drop(file);
        info!("Paper fill PnL={:.4} {}", pnl, rec.symbol);
        Ok(rec)
    }
}

#[derive(Debug, Default)]
struct WalkResult {
    qty: f64,
    notional: f64,
}

fn walk(book: &OrderBook, side: Side, target: f64) -> WalkResult {
    let it: Box<dyn Iterator<Item = (&ordered_float::OrderedFloat<f64>, &f64)> + '_> = match side {
        Side::Ask => Box::new(book.asks.iter()),
        Side::Bid => Box::new(book.bids.iter().rev()),
    };
    let mut qty = 0.0;
    let mut notional = 0.0;
    for (&p, &q) in it {
        let pf = p.into_inner();
        let lvl = pf * q;
        let need = target - notional;
        if lvl >= need {
            qty += need / pf;
            notional += need;
            break;
        } else {
            qty += q;
            notional += lvl;
        }
    }
    WalkResult { qty, notional }
}
