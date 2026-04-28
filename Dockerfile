# syntax=docker/dockerfile:1
# Local build required first: cd ~/arb_bot && cargo build --release --workspace
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY target/release/arb-bin /usr/local/bin/arb
COPY target/release/arb-recorder /usr/local/bin/
COPY target/release/arb-backtest /usr/local/bin/
COPY config.yaml ./

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9090/metrics || exit 1

CMD ["/usr/local/bin/arb", "--mode", "paper"]
