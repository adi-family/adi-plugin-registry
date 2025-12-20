FROM rust:1.83-slim-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    perl \
    libfindbin-libs-perl \
    make \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY Cargo.toml ./
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false registry

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/release/adi-plugin-registry /app/adi-plugin-registry

# Create data directory
RUN mkdir -p /data && chown registry:registry /data

USER registry

ENV PORT=8080
ENV RUST_LOG=info

EXPOSE 8080

VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/adi-plugin-registry"]
