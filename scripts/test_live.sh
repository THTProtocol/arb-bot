#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export BINANCE_TESTNET_API_KEY="gfArOWf3OAqJOiJe3waooGzKAH1p5dEl3gkCLo5KYJEd25xhX4Ka6IYyaolC7uh0"
export BINANCE_TESTNET_API_SECRET="a8lQVjGBfSMwcMdiAoEvetR5FqcUvLdWVp2HTuziOJIArSDvESjvCWjzzlbsl8QrLu1lQ=="
export OKX_DEMO_API_KEY="8cd41510-ed6f-4599-a74a-ef0f169ec3c3"
export OKX_DEMO_API_SECRET="24BC17362C80A67A6C526A62E2B009EE"
export OKX_DEMO_PASSPHRASE="mariusHermes1."

echo "=== Building ==="
cargo build --release --workspace --quiet

echo "=== Dry-run live mode (10 seconds) ==="
timeout 10 ./target/release/arb-bin \
  --config config.yaml \
  --output /tmp/live_test.jsonl \
  --venues binance,okx \
  --mode live \
  --binance-api-key "$BINANCE_TESTNET_API_KEY" \
  --binance-api-secret "$BINANCE_TESTNET_API_SECRET" \
  --okx-api-key "$OKX_DEMO_API_KEY" \
  --okx-api-secret "$OKX_DEMO_API_SECRET" \
  --okx-passphrase "$OKX_DEMO_PASSPHRASE" \
  --log-level info || true

echo "=== Done ==="
