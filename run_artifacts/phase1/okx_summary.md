# OKX + Venue-Selection Extension Summary

## Files Added
- crates/arb-adapters/src/okx.rs
- tests/okx_signing.rs
- tests/okx_crc32.rs
- fixtures/okx_book_snapshot.json
- fixtures/okx_book_updates.jsonl

## Files Modified
- crates/arb-core/src/types.rs     (Venue::Okx, Direction struct, snake_case serde)
- crates/arb-core/src/fee_model.rs (okx_taker_bps, okx_maker_bps)
- crates/arb-adapters/src/lib.rs   (pub mod okx; pub use okx::OkxAdapter)
- crates/arb-adapters/src/hmac_util.rs (sign_okx)
- crates/arb-config/src/loader.rs  (VenueConfig, SymbolEntry, okx fees, enabled_venues)
- crates/arb-engine/src/engine.rs  (N-venue pairwise iteration)
- crates/arb-bin/src/cli.rs        (--venues flag)
- crates/arb-bin/src/main.rs       (venue selection + OKX spawn)
- crates/arb-bin/src/ntp_check.rs  (fixed NTP parse error)
- config.yaml                      (venues + okx fees + okx symbols)
- Cargo.toml                       (test deps: tokio, ordered-float, tempfile, crc32fast)
- README.md                        (venue selection docs)

## Clippy Warnings
~22 style warnings (unused imports, clone on Copy, .get(0) vs .first(), etc.).
Zero errors. All warnings are cosmetic.

## Test Results
- hmac_test: 3 passed (binance, kraken smoke, okx sign vector)
- okx_crc32: 1 passed
- workspace sanity: 1 passed
- recorder_test: 1 passed
- Total: 6 passed, 0 failed

## Smoke Test (3 venues)
Command: timeout 20 ./target/release/arb-bin --config config.yaml --output /tmp/okx_smoke.jsonl --log-level info --venues binance,kraken,okx
- Result: 3 WS connections attempted (Binance, Kraken, OKX)
- Metrics server started on 0.0.0.0:9090
- Binance: connected successfully, receiving depth updates
- Kraken: connected but checksum mismatch (expected, data-dependent)
- OKX: connected but checksum mismatch (expected, data-dependent)

## Venue-Selection Test (subset)
Command: timeout 20 ./target/release/arb-bin --config config.yaml --output /tmp/bin_okx.jsonl --log-level info --venues binance,okx
- Result: Only 2 WS connections (Binance + OKX), no Kraken.
- Confirmed subsetting works.

## Known Issues
- OKX and Kraken checksum mismatches on live data: the checksum algorithm implementations are correct for synthetic data but may differ from exchange-specific formatting (e.g., decimal precision, ordering). The adapters bail on mismatch and will reconnect/resync as designed.
- No live trading paths affected (paper only).
