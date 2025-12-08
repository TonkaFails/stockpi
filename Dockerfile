# ==============================================================================
# Base Stage: Setup tools common to build stages
# ==============================================================================
FROM rust:1-bookworm AS base
# We use cargo-chef to handle dependency caching optimally
RUN cargo install cargo-chef
WORKDIR /app

# ==============================================================================
# Planner Stage: Computes the dependency recipe
# ==============================================================================
FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ==============================================================================
# Builder Stage: Builds the actual application
# ==============================================================================
FROM base AS builder

# Install build dependencies for native-tls (OpenSSL)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json

# Build dependencies only (this layer is cached if Cargo.toml/lock doesn't change)
RUN cargo chef cook --release --recipe-path recipe.json

# Build the actual application
COPY . .
RUN cargo build --release --bin stockpi

# ==============================================================================
# Runtime Stage: Minimal image for running the app
# ==============================================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime libraries for native-tls and CA certificates for HTTPS/WSS
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder
COPY --from=builder /app/target/release/stockpi /usr/local/bin/stockpi

COPY .env .

ENV RUST_LOG=info
EXPOSE 3000

CMD ["stockpi"]