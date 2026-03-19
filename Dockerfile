# Stage 1: Build
FROM rust:1.94-slim AS builder

WORKDIR /usr/src/pixcli

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/

# Build all workspace crates in release mode
RUN cargo build --workspace --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/pixcli/target/release/pixcli /usr/local/bin/pixcli

ENTRYPOINT ["pixcli"]
