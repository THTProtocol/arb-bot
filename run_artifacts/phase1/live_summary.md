# Live Trading + Strategy Training Extension — STATUS

## API Keys Stored (in memory)
- Binance Testnet: gfArOWf3OAqJOiJe3waooGzKAH1p5dEl3gkCLo5KYJEd25xhX4Ka6IYyaolC7uh0
- Binance Secret: a8lQVjGBfSMwcMdiAoEvetR5FqcUvLdWVp2HTuziOJIArSDvESjvCWjzzlbsl8QrLu1lQ==
- OKX Demo: 8cd41510-ed6f-4599-a74a-ef0f169ec3c3
- OKX Secret: 24BC17362C80A67A6C526A62E2B009EE
- OKX Passphrase: mariusHermes1.

## Crates Added
- `crates/arb-live/` — REST order clients for Binance testnet + OKX demo, LiveExecutor
- `crates/arb-train/` — Hyperparameter sweep over recorded book data, CSV report

## New CLI Flags (arb-bin)
--mode <paper|observe|live>
--binance-api-key       (env: BINANCE_TESTNET_API_KEY)
--binance-api-secret    (env: BINANCE_TESTNET_API_SECRET)
--okx-api-key           (env: OKX_DEMO_API_KEY)
--okx-api-secret        (env: OKX_DEMO_API_SECRET)
--okx-passphrase        (env: OKX_DEMO_PASSPHRASE)
--train-input <path>    (runs sweep, writes CSV report)
--train-output <path>

## Compilation
- cargo check --workspace: PASS
- cargo build --workspace: PASS
- cargo test --workspace: PASS (7 tests, 0 failures)
- Warnings: ~25 cosmetic (unused imports, snake_case on API structs, unused variable)

## How to use
Paper trading (default):
    ./target/release/arb-bin --config config.yaml --venues binance,okx

Live trading (testnet/demo):
    export BINANCE_TESTNET_API_KEY=...
    export BINANCE_TESTNET_API_SECRET=...
    export OKX_DEMO_API_KEY=...
    export OKX_DEMO_API_SECRET=...
    export OKX_DEMO_PASSPHRASE=...
    ./target/release/arb-bin --mode live --config config.yaml --venues binance,okx

Training/backtesting on recorded data:
    ./target/release/arb-bin --train-input recordings.jsonl --train-output report.csv

## Known Issues
- OKX/Binance API structs use original exchange field names (non_snake_case).
- Live mode places LIMIT orders with GTC, polls for fills immediately after placement.
  Real-world latency may require async fill tracking instead of blocking poll.
- Kraken live execution not yet implemented (returns bail).
- Training sweep uses hardcoded default grid; user-defined grid YAML not yet wired.
