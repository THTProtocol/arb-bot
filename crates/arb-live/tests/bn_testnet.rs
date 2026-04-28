use arb_live::binance_orders::BinanceClient;

#[tokio::test]
async fn test_binance_testnet_signing() {
    let key = std::env::var("BINANCE_TESTNET_API_KEY").unwrap_or_default();
    let secret = std::env::var("BINANCE_TESTNET_API_SECRET").unwrap_or_default();
    if key.is_empty() || secret.is_empty() {
        eprintln!("SKIP: env vars not set");
        return;
    }
    println!("Key len={}  Secret len={}", key.len(), secret.len());

    // Use the client to fetch server time first
    let client = reqwest::Client::new();
    let ts: serde_json::Value = client
        .get("https://testnet.binance.vision/api/v3/time")
        .send().await.unwrap()
        .json().await.unwrap();
    let server_ts = ts["serverTime"].as_i64().unwrap();
    println!("Server time = {}", server_ts);

    // Quick sign verification via the crate's internal logic
    let sig = arb_adapters::hmac_util::sign_binance(
        &format!("timestamp={}", server_ts),
        &secret,
    );
    println!("HMAC sig = {}", sig);

    // Hit /api/v3/account with the signature
    let url = format!(
        "https://testnet.binance.vision/api/v3/account?timestamp={}&signature={}",
        server_ts, sig
    );
    let resp = client
        .get(&url)
        .header("X-MBX-APIKEY", &key)
        .send().await.unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap();
    println!("GET /account status={} body={}", status, body);
    assert!(status.as_u16() == 200, "Expected 200, got {}: {}", status, body);
}
