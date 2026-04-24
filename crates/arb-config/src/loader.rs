use arb_core::fee_model::FeeModel;
use arb_core::types::{BuildMode, NormalizedSymbol, Strategy, Venue};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub mode: BuildMode,
    pub profit_threshold_bps: f64,
    pub notional_usd: f64,
    pub usd_usdt_basis_bps: f64,
    pub bnb_discount: bool,
    pub ntp_max_drift_ms: f64,
    pub venues: VenueConfig,
    pub symbols: Vec<SymbolEntry>,
    pub fees: FeesConfig,
    pub strategies: Vec<Strategy>,
    pub risk: RiskConfig,
    pub log: LogConfig,
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub api_keys: ApiKeys,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SymbolEntry {
    pub binance: Option<String>,
    pub kraken: Option<String>,
    pub okx: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VenueConfig {
    pub enabled: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeesConfig {
    pub binance: FeeEntry,
    pub kraken: FeeEntry,
    pub okx: FeeEntry,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeeEntry {
    pub taker_bps: f64,
    pub maker_bps: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RiskConfig {
    pub max_notional_per_opp_usd: f64,
    pub circuit_breaker_errors: u32,
    pub circuit_breaker_cooldown_s: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogConfig {
    pub level: String,
    pub format: String,
    pub path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetricsConfig {
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ApiKeys {
    pub binance_api_key: Option<String>,
    pub binance_api_secret: Option<String>,
    pub okx_api_key: Option<String>,
    pub okx_api_secret: Option<String>,
    pub okx_passphrase: Option<String>,
}

impl Config {
    pub fn api_keys(&self) -> ApiKeys {
        let mut keys = self.api_keys.clone();
        if keys.binance_api_key.is_none() {
            keys.binance_api_key = std::env::var("BINANCE_TESTNET_API_KEY").ok();
        }
        if keys.binance_api_secret.is_none() {
            keys.binance_api_secret = std::env::var("BINANCE_TESTNET_API_SECRET").ok();
        }
        if keys.okx_api_key.is_none() {
            keys.okx_api_key = std::env::var("OKX_DEMO_API_KEY").ok();
        }
        if keys.okx_api_secret.is_none() {
            keys.okx_api_secret = std::env::var("OKX_DEMO_API_SECRET").ok();
        }
        if keys.okx_passphrase.is_none() {
            keys.okx_passphrase = std::env::var("OKX_DEMO_PASSPHRASE").ok();
        }
        keys
    }
    pub fn fee_model(&self) -> FeeModel {
        FeeModel {
            binance_taker_bps: if self.bnb_discount {
                7.5
            } else {
                self.fees.binance.taker_bps
            },
            binance_maker_bps: if self.bnb_discount {
                7.5
            } else {
                self.fees.binance.maker_bps
            },
            kraken_taker_bps: self.fees.kraken.taker_bps,
            kraken_maker_bps: self.fees.kraken.maker_bps,
            okx_taker_bps: self.fees.okx.taker_bps,
            okx_maker_bps: self.fees.okx.maker_bps,
        }
    }

    pub fn symbol_maps(&self) -> Vec<HashMap<Venue, NormalizedSymbol>> {
        self.symbols
            .iter()
            .map(|s| {
                let mut m = HashMap::new();
                // Derive base/quote from whichever venue mapping exists
                let (base, quote) = if let Some(ref b) = s.binance {
                    let base = b[..b.len().saturating_sub(4)].to_string();
                    let quote = b[b.len().saturating_sub(4)..].to_string();
                    (base, quote)
                } else if let Some(ref k) = s.kraken {
                    let parts: Vec<&str> = k.split('/').collect();
                    (
                        parts
                            .get(0)
                            .unwrap_or(&"")
                            .to_string()
                            .replace("XBT", "BTC"),
                        parts.get(1).unwrap_or(&"").to_string(),
                    )
                } else if let Some(ref o) = s.okx {
                    let parts: Vec<&str> = o.split('-').collect();
                    (
                        parts.get(0).unwrap_or(&"").to_string(),
                        parts.get(1).unwrap_or(&"").to_string(),
                    )
                } else {
                    ("".to_string(), "".to_string())
                };
                if base.is_empty() || quote.is_empty() {
                    return m;
                }
                let norm = NormalizedSymbol::new(base.clone(), quote.clone());
                if s.binance.is_some() {
                    m.insert(Venue::Binance, norm.clone());
                }
                if s.kraken.is_some() {
                    m.insert(Venue::Kraken, norm.clone());
                }
                if s.okx.is_some() {
                    m.insert(Venue::Okx, norm.clone());
                }
                m
            })
            .collect()
    }

    pub fn enabled_venues(&self) -> Vec<Venue> {
        self.venues
            .enabled
            .iter()
            .filter_map(|s| match s.to_lowercase().as_str() {
                "binance" => Some(Venue::Binance),
                "kraken" => Some(Venue::Kraken),
                "okx" => Some(Venue::Okx),
                other => {
                    tracing::warn!("unknown venue: {}", other);
                    None
                }
            })
            .collect()
    }
}

pub fn load(path: &Path) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let cfg: Config = serde_yaml::from_str(&content)?;
    Ok(cfg)
}
