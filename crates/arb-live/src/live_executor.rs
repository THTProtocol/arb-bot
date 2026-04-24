use crate::binance_orders::{BinanceClient, PlaceOrderReq as BReq};
use crate::okx_orders::{OkxClient, PlaceOrderReq as OReq};
use arb_core::types::{Opportunity, Venue};
use tracing::{info, warn};

#[derive(Debug, Clone, serde::Serialize)]
pub struct LiveFillRecord {
    pub ts_ns: u64,
    pub symbol: String,
    pub direction: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub buy_filled_qty: f64,
    pub buy_avg_px: f64,
    pub sell_filled_qty: f64,
    pub sell_avg_px: f64,
    pub fee_buy: f64,
    pub fee_sell: f64,
    pub pnl: f64,
    pub status: String,
}

pub struct LiveExecutor {
    binance: Option<BinanceClient>,
    okx: Option<OkxClient>,
    min_fill_qty: f64,
}

impl LiveExecutor {
    pub fn new(binance: Option<BinanceClient>, okx: Option<OkxClient>) -> Self {
        Self {
            binance,
            okx,
            min_fill_qty: 0.0001,
        }
    }

    pub async fn execute(&self, opp: &Opportunity) -> anyhow::Result<LiveFillRecord> {
        let symbol_raw = format!("{}{}", opp.symbol.base, opp.symbol.quote);
        let buy_qty = opp.notional_usd / opp.buy_vwap;
        let sell_qty = opp.notional_usd / opp.sell_vwap;

        let (buy_id, buy_filled, buy_avg_px, buy_status) = match opp.direction.buy {
            Venue::Binance => {
                let client = self
                    .binance
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("binance client missing"))?;
                let req = BReq {
                    symbol: symbol_raw.clone(),
                    side: "BUY".into(),
                    order_type: "LIMIT".into(),
                    quantity: buy_qty,
                    price: Some(opp.buy_vwap),
                    time_in_force: Some("GTC".into()),
                };
                info!(
                    "Placing Binance BUY {} qty={} px={}",
                    symbol_raw, buy_qty, opp.buy_vwap
                );
                let resp = client.place_order(&req).await?;
                info!(
                    "Binance BUY placed id={} status={}",
                    resp.orderId, resp.status
                );
                let query = client.query_order(&symbol_raw, resp.orderId).await?;
                let filled: f64 = query.executedQty.parse().unwrap_or(0.0);
                let quote: f64 = query.cummulativeQuoteQty.parse().unwrap_or(0.0);
                let avg_px = if filled > 0.0 { quote / filled } else { 0.0 };
                (resp.orderId.to_string(), filled, avg_px, query.status)
            }
            Venue::Okx => {
                let client = self
                    .okx
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("okx client missing"))?;
                let req = OReq {
                    instId: format!("{}-{}", opp.symbol.base, opp.symbol.quote),
                    tdMode: "cash".into(),
                    side: "buy".into(),
                    ordType: "limit".into(),
                    sz: format!("{:.8}", buy_qty),
                    px: Some(format!("{:.8}", opp.buy_vwap)),
                };
                info!(
                    "Placing OKX BUY {} qty={} px={}",
                    req.instId, buy_qty, opp.buy_vwap
                );
                let resp = client.place_order(&req).await?;
                info!("OKX BUY placed id={}", resp.ordId);
                let query = client.query_order(&req.instId, &resp.ordId).await?;
                let filled: f64 = query.accFillSz.parse().unwrap_or(0.0);
                let avg_px: f64 = query.avgPx.parse().unwrap_or(0.0);
                (resp.ordId, filled, avg_px, query.state)
            }
            Venue::Kraken => {
                anyhow::bail!("Live execution not yet supported for Kraken");
            }
        };

        let (sell_id, sell_filled, sell_avg_px, sell_status) = match opp.direction.sell {
            Venue::Binance => {
                let client = self
                    .binance
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("binance client missing"))?;
                let req = BReq {
                    symbol: symbol_raw.clone(),
                    side: "SELL".into(),
                    order_type: "LIMIT".into(),
                    quantity: sell_qty,
                    price: Some(opp.sell_vwap),
                    time_in_force: Some("GTC".into()),
                };
                info!(
                    "Placing Binance SELL {} qty={} px={}",
                    symbol_raw, sell_qty, opp.sell_vwap
                );
                let resp = client.place_order(&req).await?;
                let query = client.query_order(&symbol_raw, resp.orderId).await?;
                let filled: f64 = query.executedQty.parse().unwrap_or(0.0);
                let quote: f64 = query.cummulativeQuoteQty.parse().unwrap_or(0.0);
                let avg_px = if filled > 0.0 { quote / filled } else { 0.0 };
                (resp.orderId.to_string(), filled, avg_px, query.status)
            }
            Venue::Okx => {
                let client = self
                    .okx
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("okx client missing"))?;
                let req = OReq {
                    instId: format!("{}-{}", opp.symbol.base, opp.symbol.quote),
                    tdMode: "cash".into(),
                    side: "sell".into(),
                    ordType: "limit".into(),
                    sz: format!("{:.8}", sell_qty),
                    px: Some(format!("{:.8}", opp.sell_vwap)),
                };
                info!(
                    "Placing OKX SELL {} qty={} px={}",
                    req.instId, sell_qty, opp.sell_vwap
                );
                let resp = client.place_order(&req).await?;
                let query = client.query_order(&req.instId, &resp.ordId).await?;
                let filled: f64 = query.accFillSz.parse().unwrap_or(0.0);
                let avg_px: f64 = query.avgPx.parse().unwrap_or(0.0);
                (resp.ordId, filled, avg_px, query.state)
            }
            Venue::Kraken => {
                anyhow::bail!("Live execution not yet supported for Kraken");
            }
        };

        if buy_filled < self.min_fill_qty || sell_filled < self.min_fill_qty {
            warn!(
                "Live fill too small buy={} sell={} — aborting",
                buy_filled, sell_filled
            );
            return Ok(LiveFillRecord {
                ts_ns: opp.ts_ns,
                symbol: symbol_raw,
                direction: format!("{}", opp.direction),
                buy_order_id: buy_id,
                sell_order_id: sell_id,
                buy_filled_qty: buy_filled,
                buy_avg_px,
                sell_filled_qty: sell_filled,
                sell_avg_px,
                fee_buy: 0.0,
                fee_sell: 0.0,
                pnl: 0.0,
                status: "underfilled".into(),
            });
        }

        let fee_buy = buy_avg_px * buy_filled * 0.001;
        let fee_sell = sell_avg_px * sell_filled * 0.0016;
        let cost = buy_avg_px * buy_filled + fee_buy;
        let revenue = sell_avg_px * sell_filled - fee_sell;
        let pnl = revenue - cost;

        info!(
            "Live PnL={:.4} buy@{} fill={} sell@{} fill={}",
            pnl, buy_avg_px, buy_filled, sell_avg_px, sell_filled
        );

        Ok(LiveFillRecord {
            ts_ns: opp.ts_ns,
            symbol: symbol_raw,
            direction: format!("{}", opp.direction),
            buy_order_id: buy_id,
            sell_order_id: sell_id,
            buy_filled_qty: buy_filled,
            buy_avg_px,
            sell_filled_qty: sell_filled,
            sell_avg_px,
            fee_buy,
            fee_sell,
            pnl,
            status: format!("{} / {}", buy_status, sell_status),
        })
    }
}
