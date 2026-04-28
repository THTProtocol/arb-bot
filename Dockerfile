# syntax=docker/dockerfile:1
FROM rust:1.78-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY config.yaml ./

RUN cargo build --release --bin arb-bin --bin arb-recorder --bin arb-backtest 2>&1 | tail -5

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/arb-bin /usr/local/bin/arb
COPY --from=builder /app/target/release/arb-recorder /usr/local/bin/
COPY --from=builder /app/target/release/arb-backtest /usr/local/bin/
COPY config.yaml ./

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9090/metrics || exit 1

CMD ["/usr/local/bin/arb", "--mode", "paper"]
