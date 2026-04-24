# Arbitrage Bot — Quick Start Guide

## GitHub Repository
https://github.com/THTProtocol/arb-bot

---

## Capital Requirements

Since this is **SPOT arbitrage**, you need BOTH assets on BOTH exchanges:

| Symbol | Required on Buy Venue | Required on Sell Venue |
|---|---|---|
| BTC/USDT | ~$1000 USDT | ~0.015 BTC |
| ETH/USDT | ~$1000 USDT | ~0.50 ETH |
| SOL/USDT | ~$1000 USDT | ~7 SOL |

**For testnet/demo:** Binance testnet faucet gives free test USDT. OKX demo accounts start with virtual funds.

---

## Configuration (config.yaml)

```yaml
mode: test              # test | paper | live
profit_threshold_bps: 15
notional_usd: 1000     # <-- $1000 max per trade
usd_usdt_basis_bps: 0
bnb_discount: false
ntp_max_drift_ms: 500
venues:
  enabled: [binance, okx]
symbols:
  - { binance: BTCUSDT, okx: "BTC-USDT" }
  - { binance: ETHUSDT, okx: "ETH-USDT" }
  - { binance: SOLUSDT, okx: "SOL-USDT" }
fees:
  binance: { taker_bps: 10.0, maker_bps: 10.0 }
  okx:     { taker_bps: 10.0, maker_bps:  8.0 }
strategies: [taker_taker, maker_taker]
risk:
  max_notional_per_opp_usd: 1000
  circuit_breaker_errors: 5
  circuit_breaker_cooldown_s: 60
log:
  level: info
  format: json
  path: ./logs/
metrics:
  port: 9090
api_keys:
  binance_api_key: ""
  binance_api_secret: ""
  okx_api_key: ""
  okx_api_secret: ""
  okx_passphrase: ""
```

---

## Running the Bot

### 1. Testnet/Demo Mode (safe, uses test API keys)
```bash
export BINANCE_TESTNET_API_KEY=gfArOWf3OA...
export BINANCE_TESTNET_API_SECRET=a8lQVjGB...
export OKX_DEMO_API_KEY=8cd41510...
export OKX_DEMO_API_SECRET=24BC1736...
export OKX_DEMO_PASSPHRASE=mariusHermes1.

./target/release/arb-bin \
  --config config.yaml \
  --mode test \
  --venues binance,okx \
  --log-level info
```

### 2. Training / Backtesting
```bash
./target/release/arb-bin \
  --config config.yaml \
  --train-input recordings.jsonl \
  --train-output report.csv
```

### 3. Live Mode with Telegram Alerts
```bash
export TELEGRAM_BOT_TOKEN=xxx
export TELEGRAM_CHAT_ID=xxx

./target/release/arb-bin \
  --config config.yaml \
  --mode live \
  --venues binance,okx \
  --tg-bot-token $TELEGRAM_BOT_TOKEN \
  --tg-chat-id $TELEGRAM_CHAT_ID
```

---

## What Each Mode Does

| Mode | WebSocket | Order Placement | Risk |
|---|---|---|---|
| observe | Yes | No (only logs opportunities) | None |
| paper | Yes | Simulated fills to JSONL ledger | None |
| test | Yes | Real orders on testnet/demo | Test funds only |
| live | Yes | Real orders on mainnet | Real money |

---

## Telegram Notifications

The bot sends two types of messages:
1. **Online alert** when bot starts, showing mode + venues
2. **Trade alert** on every live/test fill showing PnL + prices

Format your Telegram bot token as: `123456:ABC-DEF...`
Find your chat ID with @userinfobot

---

## Auto-Training

The training sweep automatically searches the best parameters:
- profit_threshold_bps: 5, 10, 20
- notional_usd: 100, 500, 1000
- slippage_alpha: 0.01, 0.05, 0.10
- strategies: taker_taker, maker_taker, both

Results go to `report.csv` with columns:
`profit_threshold_bps, notional_usd, strategies, num_opps, avg_net_bps, sharpe, max_drawdown_bps, win_rate`

To use best params, update your config.yaml with the winning row.
