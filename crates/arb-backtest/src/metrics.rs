use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize)]
pub struct RunReport {
    pub version: String,
    pub config_hash: String,
    pub n_updates: usize,
    pub n_opportunities: usize,
    pub n_fills: usize,
    pub fill_rate_pct: f64,
    pub gross_pnl: f64,
    pub net_pnl: f64,
    pub sharpe: f64,
    pub max_drawdown_pct: f64,
    pub realized_slippage_bps: f64,
    pub per_symbol: HashMap<String, SymbolMetrics>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SymbolMetrics {
    pub opp_count: usize,
    pub fills: usize,
    pub gross_pnl: f64,
    pub net_pnl: f64,
}

use arb_exec::paper_executor::FillRecord;

impl RunReport {
    pub fn compute(records: &[FillRecord], version: &str, config_hash: &str, n_updates: usize) -> Self {
        let n_fills = records.len();
        let mut gross_pnl = 0.0;
        let mut net_pnl = 0.0;
        let mut pnl_series: Vec<f64> = Vec::with_capacity(n_fills);
        let mut peak = 0.0_f64;
        let mut max_dd = 0.0_f64;
        let mut per_symbol: HashMap<String, SymbolMetrics> = HashMap::new();
        let mut gross_slippage = 0.0;

        for rec in records {
            gross_pnl += rec.sim_pnl + rec.fee_buy + rec.fee_sell;
            net_pnl += rec.sim_pnl;
            pnl_series.push(rec.sim_pnl);
            let cum: f64 = pnl_series.iter().sum();
            if cum > peak {
                peak = cum;
            }
            let dd = peak - cum;
            if dd > max_dd {
                max_dd = dd;
            }
            let sym = rec.symbol.clone();
            let entry = per_symbol.entry(sym).or_default();
            entry.fills += 1;
            entry.gross_pnl += rec.sim_pnl + rec.fee_buy + rec.fee_sell;
            entry.net_pnl += rec.sim_pnl;
        }

        let mean = if !pnl_series.is_empty() {
            pnl_series.iter().sum::<f64>() / pnl_series.len() as f64
        } else {
            0.0
        };
        let variance = if pnl_series.len() > 1 {
            pnl_series.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / pnl_series.len() as f64
        } else {
            0.0
        };
        let stddev = variance.sqrt();
        let sharpe = if stddev > 1e-12 {
            mean / stddev
        } else {
            0.0
        };

        let fill_rate = if n_updates > 0 {
            (n_fills as f64 / n_updates as f64) * 100.0
        } else {
            0.0
        };

        for rec in records {
            let slip_bps = ((rec.sell_price - rec.buy_price) / rec.buy_price).abs() * 10_000.0;
            gross_slippage += slip_bps;
        }
        let slippage_bps = if n_fills > 0 { gross_slippage / n_fills as f64 } else { 0.0 };

        RunReport {
            version: version.to_string(),
            config_hash: config_hash.to_string(),
            n_updates,
            n_opportunities: n_fills, // opps that passed threshold
            n_fills,
            fill_rate_pct: fill_rate,
            gross_pnl,
            net_pnl,
            sharpe,
            max_drawdown_pct: if peak > 1e-12 { (max_dd / peak) * 100.0 } else { 0.0 },
            realized_slippage_bps: slippage_bps,
            per_symbol,
        }
    }
}
