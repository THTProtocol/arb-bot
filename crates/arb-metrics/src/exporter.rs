use anyhow::Result;
use arb_core::types::{Direction, Strategy, Venue};
use axum::{extract::State, routing::get, Router};
use prometheus::{
    gather, register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, Encoder,
    GaugeVec, HistogramVec, TextEncoder,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct Metrics {
    pub opportunities_total: CounterVec,
    pub book_update_latency_ms: HistogramVec,
    pub engine_eval_latency_us: HistogramVec,
    pub net_bps: HistogramVec,
    pub ws_connected: GaugeVec,
    pub book_age_ms: GaugeVec,
    pub inventory_usd: GaugeVec,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            opportunities_total: register_counter_vec!(
                "opportunities_total",
                "Opportunities detected",
                &["strategy", "direction", "symbol"]
            )
            .unwrap(),
            book_update_latency_ms: register_histogram_vec!(
                "book_update_latency_ms",
                "Latency from exchange to engine",
                &["venue"],
                vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]
            )
            .unwrap(),
            engine_eval_latency_us: register_histogram_vec!(
                "engine_eval_latency_us",
                "Engine evaluation time",
                &["symbol"],
                vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]
            )
            .unwrap(),
            net_bps: register_histogram_vec!(
                "net_bps",
                "Net bps of opportunities",
                &["symbol"],
                vec![0.0, 5.0, 10.0, 15.0, 20.0, 30.0, 50.0, 100.0]
            )
            .unwrap(),
            ws_connected: register_gauge_vec!("ws_connected", "WS connection status", &["venue"])
                .unwrap(),
            book_age_ms: register_gauge_vec!("book_age_ms", "Book age in ms", &["venue", "symbol"])
                .unwrap(),
            inventory_usd: register_gauge_vec!("inventory_usd", "Paper inventory USD", &["venue"])
                .unwrap(),
        }
    }
}

pub async fn start(metrics: Arc<Metrics>, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Metrics server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> String {
    let gathered = gather();
    let encoder = TextEncoder::new();
    let mut buf = Vec::new();
    encoder.encode(&gathered, &mut buf).unwrap_or_default();
    String::from_utf8(buf).unwrap_or_default()
}
