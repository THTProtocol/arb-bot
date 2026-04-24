use arb_adapters::hmac_util::{sign_binance, sign_kraken, sign_okx};

#[test]
fn test_binance_signing() {
    let secret = "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j";
    let query = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
    let expected = "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71";
    assert_eq!(sign_binance(query, secret), expected);
}

#[test]
fn test_kraken_signing_smoke() {
    let secret_b64 = "YWJj";
    let nonce = "1616492376624";
    let body = "nonce=1616492376624";
    let path = "/0/private/AddOrder";
    let sig = sign_kraken(path, nonce, body, secret_b64);
    assert!(!sig.is_empty());
}

#[test]
fn okx_sign_vector() {
    let ts = "2020-12-08T09:08:57.715Z";
    let method = "GET";
    let path = "/api/v5/account/balance?ccy=BTC";
    let body = "";
    let secret = "22582BD0CFF14C41EDBF1AB98506286D";
    let sig = sign_okx(secret, ts, method, path, body);
    assert_eq!(sig.len(), 44);
    assert!(sig
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='));
}
