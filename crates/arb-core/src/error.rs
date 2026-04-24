use thiserror::Error;

/// Top-level error type for the arbitrage bot.
#[derive(Error, Debug)]
pub enum ArbError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("WebSocket error: {0}")]
    Ws(String),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Book sequence gap detected: {0}")]
    BookGap(String),
    #[error("Signature error: {0}")]
    Signature(String),
    #[error("NTP drift exceeds threshold: {0} ms")]
    NtpDrift(f64),
    #[error("Circuit breaker open for venue {venue}")]
    CircuitBreaker { venue: String },
    #[error("Unknown symbol: {0}")]
    UnknownSymbol(String),
    #[error("Exchange API error: {0}")]
    ExchangeApi(String),
    #[error("{0}")]
    Other(String),
}
