use crate::types::NormalizedSymbol;
use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;

/// Kraken AssetPairs response.
#[derive(Debug, Deserialize)]
struct AssetPairsResp {
    #[serde(flatten)]
    pairs: HashMap<String, AssetPairInfo>,
}

#[derive(Debug, Deserialize)]
struct AssetPairInfo {
    altname: String,
    wsname: Option<String>,
    base: String,
    quote: String,
}

/// Maps between venue-specific symbol names and normalized symbols.
#[derive(Debug, Clone, Default)]
pub struct SymbolMap {
    /// binance symbol -> normalized
    binance: HashMap<String, NormalizedSymbol>,
    /// kraken wsname -> normalized
    kraken_ws: HashMap<String, NormalizedSymbol>,
    /// kraken altname -> normalized
    kraken_alt: HashMap<String, NormalizedSymbol>,
    /// normalized -> kraken wsname
    norm_to_kraken: HashMap<NormalizedSymbol, String>,
    /// normalized -> binance symbol
    norm_to_binance: HashMap<NormalizedSymbol, String>,
}

impl SymbolMap {
    pub async fn fetch(reqwest_client: &reqwest::Client) -> anyhow::Result<Self> {
        let resp: AssetPairsResp = reqwest_client
            .get("https://api.kraken.com/0/public/AssetPairs")
            .send()
            .await?
            .json()
            .await?;

        let mut map = Self::default();
        for (_key, info) in resp.pairs {
            let base = normalize_base(&info.base);
            let quote = normalize_quote(&info.quote);
            let norm = NormalizedSymbol::new(base.clone(), quote.clone());

            if let Some(ws) = info.wsname {
                map.kraken_ws.insert(ws.clone(), norm.clone());
                map.norm_to_kraken.insert(norm.clone(), ws);
            }
            map.kraken_alt.insert(info.altname.clone(), norm.clone());
            map.norm_to_kraken
                .entry(norm.clone())
                .or_insert_with(|| info.altname.clone());
        }
        Ok(map)
    }

    pub fn add_binance(&mut self, binance_sym: &str, kraken_ws: &str) -> anyhow::Result<()> {
        let parts: Vec<&str> = binance_sym.split("USDT").collect();
        anyhow::ensure!(
            parts.len() == 2 && parts[1].is_empty(),
            "expected symbol like BTCUSDT"
        );
        let base = normalize_base(parts[0]);
        let norm = NormalizedSymbol::new(base.clone(), "USDT".to_string());
        self.binance.insert(binance_sym.to_string(), norm.clone());
        self.norm_to_binance
            .insert(norm.clone(), binance_sym.to_string());
        self.kraken_ws.insert(kraken_ws.to_string(), norm.clone());
        self.norm_to_kraken
            .entry(norm)
            .or_insert_with(|| kraken_ws.to_string());
        Ok(())
    }

    pub fn binance_to_norm(&self, binance_sym: &str) -> Option<&NormalizedSymbol> {
        self.binance.get(binance_sym)
    }

    pub fn kraken_ws_to_norm(&self, wsname: &str) -> Option<&NormalizedSymbol> {
        self.kraken_ws.get(wsname)
    }

    pub fn norm_to_binance(&self, norm: &NormalizedSymbol) -> Option<&String> {
        self.norm_to_binance.get(norm)
    }

    pub fn norm_to_kraken_ws(&self, norm: &NormalizedSymbol) -> Option<&String> {
        self.norm_to_kraken.get(norm)
    }
}

fn normalize_base(raw: &str) -> String {
    if raw.eq_ignore_ascii_case("XBT") {
        "BTC".to_string()
    } else {
        raw.to_ascii_uppercase()
    }
}

fn normalize_quote(raw: &str) -> String {
    raw.to_ascii_uppercase()
}
