# Build stage
FROM rust:1.75-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Copy OpenZeppelin Monitor (assuming it's in parent directory)
# In production, you'd use a git submodule or published crate
COPY ../openzeppelin-monitor ../openzeppelin-monitor

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash monitor

# Copy binary from builder
COPY --from=builder /app/target/release/oz-monitor-orchestrator /usr/local/bin/

# Create config directory
RUN mkdir -p /etc/oz-monitor && chown monitor:monitor /etc/oz-monitor

# Switch to non-root user
USER monitor

# Expose metrics port
EXPOSE 3000

# Default to worker mode
ENV SERVICE_MODE=worker

# Run the orchestrator
ENTRYPOINT ["oz-monitor-orchestrator"]