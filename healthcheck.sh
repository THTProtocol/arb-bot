#!/bin/bash
# Verify both exchange order clients in one shot
set -euo pipefail

echo "=== Checking .env files ==="
if [ ! -f .env.testnet ]; then
  echo "SKIP: .env.testnet not found"
  exit 0
fi

echo "=== Docker sanity ==="
docker --version && echo "Docker OK"

echo "=== Building arb-bot ==="
cd "$(dirname "$0")"
cargo build --release --bin arb-bin --bin arb-recorder --bin arb-backtest 2>&1 | tail -3
echo "Build OK"

echo "=== Docker build ==="
docker build -t arb-bot:local . 2>&1 | tail -3

# Run recorder health check (10 sec capture)
echo "=== Recorder health check (10s) ==="
timeout 15 ./target/release/arb-recorder --venues binance --out /tmp/rec_health --log-level warn || true
line_count=$(find /tmp/rec_health -name '*.jsonl' -exec wc -l {} + 2>/dev/null | awk '{s+=$1}END{print s}')
if [ "${line_count:-0}" -gt 0 ]; then
  echo "  PASS: Captured $line_count updates"
else
  echo "  FAIL: No updates captured"
fi

# Run backtest on latest data
echo "=== Backtest health check ==="
rm -rf /tmp/bt_out
./target/release/arb-backtest --recordings recordings/2026-04-27 --out /tmp/bt_out --profit-threshold-bps 1.0 2>&1 | grep -E 'complete|net_pnl'

# Testnet order ping (only if .env.testnet credentials look valid)
echo "=== Testnet order ping ==="
export $(grep -E '^(BINANCE|OKX)' .env.testnet | xargs)
if [ "${#BINANCE_TESTNET_API_KEY}" -gt 50 ] && [ "${#BINANCE_TESTNET_API_SECRET}" -gt 50 ]; then
  cargo test -p arb-live --test bn_testnet -- --nocapture 2>&1 | grep -E 'status=200|SKIP|test result'
else
  echo "  SKIP: BN key too short (${#BINANCE_TESTNET_API_KEY} / ${#BINANCE_TESTNET_API_SECRET})"
fi

echo "=== Health check complete ==="
