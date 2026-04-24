use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Grid {
    pub profit_threshold_bps: Vec<f64>,
    pub notional_usd: Vec<f64>,
    pub usd_usdt_basis_bps: Vec<f64>,
    pub slippage_alpha: Vec<f64>,
    pub strategies: Vec<Vec<String>>,
}

impl Grid {
    pub fn combos(&self) -> Vec<Combo> {
        let mut out = Vec::new();
        for pt in &self.profit_threshold_bps {
            for nu in &self.notional_usd {
                for ub in &self.usd_usdt_basis_bps {
                    for sa in &self.slippage_alpha {
                        for s in &self.strategies {
                            out.push(Combo {
                                profit_threshold_bps: *pt,
                                notional_usd: *nu,
                                usd_usdt_basis_bps: *ub,
                                slippage_alpha: *sa,
                                strategies: s.clone(),
                            });
                        }
                    }
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Combo {
    pub profit_threshold_bps: f64,
    pub notional_usd: f64,
    pub usd_usdt_basis_bps: f64,
    pub slippage_alpha: f64,
    pub strategies: Vec<String>,
}
