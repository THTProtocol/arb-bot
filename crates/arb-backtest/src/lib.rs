pub mod metrics;
pub mod replay;
pub mod simulator;

use arb_config::loader::Config;
use arb_core::types::{NormalizedSymbol, Venue};
use arb_engine::engine::EngineConfig;
use std::collections::HashMap;

/// Build EngineConfig from Config with explicit overrides.
pub fn engine_config_from(
    cfg: &Config,
    symbol_maps: Vec<HashMap<Venue, NormalizedSymbol>>,
    enabled_venues: Vec<Venue>,
    slippage_alpha: f64,
    profit_override: Option<f64>,
    notional_override: Option<f64>,
) -> EngineConfig {
    EngineConfig {
        mode: cfg.mode.clone(),
        profit_threshold_bps: profit_override.unwrap_or(cfg.profit_threshold_bps),
        notional_usd: notional_override.unwrap_or(cfg.notional_usd),
        usd_usdt_basis_bps: cfg.usd_usdt_basis_bps,
        strategies: cfg.strategies.clone(),
        symbol_maps,
        enabled_venues,
        fee_model: cfg.fee_model(),
        slippage_alpha,
    }
}
