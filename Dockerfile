FROM rust:1.78-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --workspace

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/arb-bin /usr/local/bin/arb-bot
COPY config.yaml .
EXPOSE 9090
ENTRYPOINT ["arb-bot"]
CMD ["--config", "config.yaml"]
