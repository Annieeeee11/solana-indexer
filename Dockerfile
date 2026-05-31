# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release --features sqlite,postgres

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/solana-indexer /usr/local/bin/solana-indexer
ENV RUST_LOG=info,solana_indexer=info
EXPOSE 8080
CMD ["solana-indexer", "start"]
