use arb_core::types::{BookUpdate, NormalizedSymbol, Venue};
use arb_recording::recorder::Recorder;
use ordered_float::OrderedFloat;
use std::path::Path;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

#[test]
fn test_roundtrip() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let file = NamedTempFile::new().unwrap();
        let recorder = Recorder::new(file.path());
        let update = BookUpdate {
            venue: Venue::Binance,
            symbol: NormalizedSymbol::new("BTC", "USDT"),
            ts_ns: 1_000_000,
            seq: 0,
            bids: vec![(OrderedFloat(60_000.0), 0.5)],
            asks: vec![(OrderedFloat(60_100.0), 1.0)],
            is_snapshot: true,
        };
        recorder.record(&update).await.unwrap();
        let content = tokio::fs::read_to_string(file.path()).await.unwrap();
        assert!(content.contains("BTC"));
    });
}
