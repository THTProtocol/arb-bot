//! OKX Demo REST order client.
use arb_adapters::hmac_util::sign_okx;
use serde::{Deserialize, Serialize};

const BASE: &str = "https://www.okx.com";

#[derive(Debug, Clone)]
pub struct OkxClient {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
    passphrase: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceOrderReq {
    pub instId: String,
    pub tdMode: String,     // cash for spot
    pub side: String,       // buy / sell
    pub ordType: String,    // limit / market
    pub sz: String,         // size
    pub px: Option<String>, // price for limit
}

#[derive(Debug, Clone, Deserialize)]
pub struct OkxData<T> {
    pub code: String,
    pub msg: String,
    pub data: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaceOrderData {
    pub ordId: String,
    pub clOrdId: String,
    pub tag: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryOrderData {
    pub ordId: String,
    pub instId: String,
    pub state: String, // live, partially_filled, filled, canceled
    pub sz: String,
    pub px: String,
    pub avgPx: String,
    pub accFillSz: String,
    pub side: String,
}

impl OkxClient {
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        passphrase: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            passphrase: passphrase.into(),
        }
    }

    fn headers(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> anyhow::Result<reqwest::header::HeaderMap> {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let sig = sign_okx(&self.api_secret, &ts, method, path, body);
        let mut hm = reqwest::header::HeaderMap::new();
        hm.insert("OK-ACCESS-KEY", self.api_key.parse()?);
        hm.insert("OK-ACCESS-SIGN", sig.parse()?);
        hm.insert("OK-ACCESS-TIMESTAMP", ts.parse()?);
        hm.insert("OK-ACCESS-PASSPHRASE", self.passphrase.parse()?);
        hm.insert("Content-Type", "application/json".parse()?);
        Ok(hm)
    }

    pub async fn place_order(&self, req: &PlaceOrderReq) -> anyhow::Result<PlaceOrderData> {
        let body = serde_json::to_string(req)?;
        let url = format!("{}{}", BASE, "/api/v5/trade/order");
        let resp = self
            .client
            .post(&url)
            .headers(self.headers("POST", "/api/v5/trade/order", &body)?)
            .body(body)
            .send()
            .await?;
        let txt = resp.text().await?;
        let parsed: OkxData<PlaceOrderData> = serde_json::from_str(&txt)
            .map_err(|e| anyhow::anyhow!("okx place_order parse err {}: {}", e, txt))?;
        if parsed.code != "0" {
            anyhow::bail!("okx order error {}: {}", parsed.code, parsed.msg);
        }
        parsed
            .data
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("okx empty order response"))
    }

    pub async fn query_order(&self, inst_id: &str, ord_id: &str) -> anyhow::Result<QueryOrderData> {
        let path = format!("/api/v5/trade/order?instId={}&ordId={}", inst_id, ord_id);
        let url = format!("{}{}", BASE, path);
        let resp = self
            .client
            .get(&url)
            .headers(self.headers("GET", &path, "")?)
            .send()
            .await?;
        let txt = resp.text().await?;
        let parsed: OkxData<QueryOrderData> = serde_json::from_str(&txt)
            .map_err(|e| anyhow::anyhow!("okx query_order parse err {}: {}", e, txt))?;
        if parsed.code != "0" {
            anyhow::bail!("okx query error {}: {}", parsed.code, parsed.msg);
        }
        parsed
            .data
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("okx empty query response"))
    }

    pub async fn cancel_order(&self, inst_id: &str, ord_id: &str) -> anyhow::Result<()> {
        let path = "/api/v5/trade/cancel-order";
        let body = format!("{{\"instId\":\"{}\",\"ordId\":\"{}\"}}", inst_id, ord_id);
        let url = format!("{}{}", BASE, path);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers("POST", path, &body)?)
            .body(body)
            .send()
            .await?;
        let txt = resp.text().await?;
        let parsed: OkxData<PlaceOrderData> = serde_json::from_str(&txt)
            .map_err(|e| anyhow::anyhow!("okx cancel parse err {}: {}", e, txt))?;
        if parsed.code != "0" {
            anyhow::bail!("okx cancel error {}: {}", parsed.code, parsed.msg);
        }
        Ok(())
    }
}
