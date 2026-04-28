//! Binance Testnet Spot REST order client.
use arb_adapters::hmac_util::sign_binance;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const BASE: &str = "https://testnet.binance.vision/api/v3";

#[derive(Debug, Clone)]
pub struct BinanceClient {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
}

#[derive(Debug, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct PlaceOrderReq {
    pub symbol: String,
    pub side: String,       // BUY or SELL
    pub order_type: String, // LIMIT or MARKET
    pub quantity: f64,
    pub price: Option<f64>,
    pub time_in_force: Option<String>, // GTC for limit
}

#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
pub struct PlaceOrderResp {
    pub symbol: String,
    pub orderId: u64,
    pub clientOrderId: String,
    pub transactTime: u64,
    pub status: String,
    pub executedQty: String,
    pub cummulativeQuoteQty: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
pub struct QueryOrderResp {
    pub symbol: String,
    pub orderId: u64,
    pub status: String,
    pub executedQty: String,
    pub cummulativeQuoteQty: String,
    pub price: String,
    pub side: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestOrderResp {
    pub dummy: Option<String>,
}

impl BinanceClient {
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            api_secret: api_secret.into(),
        }
    }

    async fn signed_query(
        &self,
        method: reqwest::Method,
        path: &str,
        mut params: HashMap<String, String>,
    ) -> anyhow::Result<reqwest::Response> {
        let ts = chrono::Utc::now().timestamp_millis();
        params.insert("timestamp".into(), ts.to_string());
        params.insert("recvWindow".into(), "10000".into());

        let mut parts: Vec<_> = params.iter().map(|(k,v)| format!("{}={}", k, v)).collect();
        parts.sort();
        let mut query = parts.join("&");
        let sig = sign_binance(&query, &self.api_secret);
        query.push_str(&format!("&signature={}", sig));

        let url = format!("{}{}?{}", BASE, path, query);
        let req = self
            .client
            .request(method, &url)
            .header("X-MBX-APIKEY", &self.api_key);
        let resp = req.send().await?;
        Ok(resp)
    }

    pub async fn place_order(&self, req: &PlaceOrderReq) -> anyhow::Result<PlaceOrderResp> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("symbol".into(), req.symbol.clone());
        params.insert("side".into(), req.side.clone());
        params.insert("type".into(), req.order_type.clone());
        params.insert("quantity".into(), format!("{:.8}", req.quantity));
        if let Some(p) = req.price {
            params.insert("price".into(), format!("{:.8}", p));
            params.insert(
                "timeInForce".into(),
                req.time_in_force.clone().unwrap_or_else(|| "GTC".into()),
            );
        }
        let resp = self
            .signed_query(reqwest::Method::POST, "/order", params)
            .await?;
        let txt = resp.text().await?;
        let order: PlaceOrderResp = serde_json::from_str(&txt)
            .map_err(|e| anyhow::anyhow!("binance place_order parse err {}: {}", e, txt))?;
        Ok(order)
    }

    pub async fn query_order(&self, symbol: &str, order_id: u64) -> anyhow::Result<QueryOrderResp> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("symbol".into(), symbol.into());
        params.insert("orderId".into(), order_id.to_string());
        let resp = self
            .signed_query(reqwest::Method::GET, "/order", params)
            .await?;
        let txt = resp.text().await?;
        let order: QueryOrderResp = serde_json::from_str(&txt)
            .map_err(|e| anyhow::anyhow!("binance query_order parse err {}: {}", e, txt))?;
        Ok(order)
    }

    pub async fn test_order(&self, req: &PlaceOrderReq) -> anyhow::Result<TestOrderResp> {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("symbol".into(), req.symbol.clone());
        params.insert("side".into(), req.side.clone());
        params.insert("type".into(), req.order_type.clone());
        params.insert("quantity".into(), format!("{:.8}", req.quantity));
        if let Some(p) = req.price {
            params.insert("price".into(), format!("{:.8}", p));
            params.insert("timeInForce".into(), "GTC".into());
        }
        let resp = self
            .signed_query(reqwest::Method::POST, "/order/test", params)
            .await?;
        let txt = resp.text().await?;
        if txt.trim() == "{}" {
            Ok(TestOrderResp { dummy: None })
        } else {
            let order: TestOrderResp = serde_json::from_str(&txt)
                .map_err(|e| anyhow::anyhow!("binance test_order parse err {}: {}", e, txt))?;
            Ok(order)
        }
    }
}
