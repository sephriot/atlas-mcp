# Build stage
FROM rust:slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage (Ubuntu 24.04 has glibc 2.39 matching rust:slim)
FROM ubuntu:24.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/atlas-mcp /usr/local/bin/atlas-mcp

# Create data directory for volume mount
RUN mkdir -p /data

# Set storage path
ENV ATLAS_STORAGE=/data

# Expose default HTTP port
EXPOSE 3333

# Run HTTP server by default
ENTRYPOINT ["atlas-mcp"]
CMD ["--http", "3333"]
