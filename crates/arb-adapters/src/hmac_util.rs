use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha512};

/// OKX signature: base64(HMAC-SHA256(secret, timestamp + method.upper() + path + (body or "")))
pub fn sign_okx(secret: &str, timestamp: &str, method: &str, path: &str, body: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let pre_sign = format!(
        "{}{}{}{}",
        timestamp,
        method.to_ascii_uppercase(),
        path,
        body
    );
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(pre_sign.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}
pub fn sign_binance(query: &str, secret: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(query.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Kraken signature: base64(HMAC-SHA512(secret, path || SHA256(nonce || body))).
pub fn sign_kraken(path: &str, nonce: &str, body: &str, secret_b64: &str) -> String {
    let secret = STANDARD.decode(secret_b64).expect("valid base64 secret");

    let mut sha256 = Sha256::new();
    sha256.update(nonce.as_bytes());
    sha256.update(body.as_bytes());
    let sha256_result = sha256.finalize();

    let mut hmac_input = path.as_bytes().to_vec();
    hmac_input.extend_from_slice(&sha256_result);

    type HmacSha512 = Hmac<Sha512>;
    let mut mac = HmacSha512::new_from_slice(&secret).expect("HMAC accepts any key length");
    mac.update(&hmac_input);
    STANDARD.encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_signing() {
        let secret = "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j";
        let query = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
        let expected = "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71";
        assert_eq!(sign_binance(query, secret), expected);
    }

    #[test]
    fn test_kraken_signing() {
        let secret_b64 = "YWJj"; // base64("abc")
        let nonce = "1616492376624";
        let body = "nonce=1616492376624";
        let path = "/0/private/AddOrder";
        let sig = sign_kraken(path, nonce, body, secret_b64);
        assert!(!sig.is_empty());
    }
}
