# Arb Bot

Cross-exchange arbitrage opportunity detector for Binance, Kraken, and OKX.

## Venue Selection

The bot supports any 2+ of: `binance`, `kraken`, `okx`.

Edit `config.yaml`:

```yaml
venues:
  enabled: [binance, okx]      # disable kraken
```

Or override at runtime:

```bash
./target/release/arb-bot --mode paper --config config.yaml --venues binance,okx
```

API key environment variables (OBSERVE mode only):
- `BINANCE_API_KEY`, `BINANCE_API_SECRET`
- `KRAKEN_API_KEY`, `KRAKEN_API_SECRET`
- `OKX_API_KEY`, `OKX_API_SECRET`, `OKX_API_PASSPHRASE`

OKX notes:
- Not accessible from US IPs; use a non-US VPS for deployment.
- Passphrase is a THIRD credential (alongside key+secret) set when creating the API key in OKX UI.
- Available from Turkey, EU, APAC regions.

## Build

```bash
cargo build --release --workspace
```

## Run

```bash
./target/release/arb-bot --config config.yaml
```

## Docker

```bash
docker-compose up --build
```

## Architecture

- `arb-core`: Types, errors, fee model, symbol map
- `arb-book`: Order book with VWAP
- `arb-adapters`: Binance, Kraken, and OKX WebSocket adapters
- `arb-engine`: Opportunity detection engine (N-venue pairwise)
- `arb-exec`: Paper executor and inventory tracking
- `arb-log`: Opportunity logger
- `arb-metrics`: Prometheus metrics exporter
- `arb-recording`: Book update recorder and replayer
- `arb-config`: YAML configuration loader
- `arb-bin`: CLI binary

## License

MIT
