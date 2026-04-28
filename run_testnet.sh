#!/bin/bash
# Wire testnet credentials from .env.testnet into the correct env var names
set -a
source ~/arb_bot/.env.testnet
export BINANCE_TESTNET_API_KEY="$BINANCE_TESTNET_KEY"
export BINANCE_TESTNET_API_SECRET="$BINANCE_TESTNET_SECRET"
export OKX_DEMO_API_KEY="$OKX_TESTNET_KEY"
export OKX_DEMO_API_SECRET="$OKX_TESTNET_SECRET"
export OKX_DEMO_PASSPHRASE="$OKX_TESTNET_PASSPHRASE"
set +a
cd ~/arb_bot
exec cargo run -p arb-bin --bin arb-bin -- --mode live --venues binance,okx --log-level info "$@"
