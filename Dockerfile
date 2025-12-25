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

# Copy lib-plugin-manifest with standalone Cargo.toml
COPY crates/lib-plugin-manifest/src /build/lib-plugin-manifest/src
RUN echo '[package]\nname = "lib-plugin-manifest"\nversion = "0.8.4"\nedition = "2021"\n\n[dependencies]\nserde = { version = "1.0", features = ["derive"] }\ntoml = "0.8"\nsemver = { version = "1", features = ["serde"] }\nthiserror = "2"' > /build/lib-plugin-manifest/Cargo.toml

# Copy lib-plugin-registry with standalone Cargo.toml
COPY crates/lib-plugin-registry/src /build/lib-plugin-registry/src
RUN echo '[package]\nname = "lib-plugin-registry"\nversion = "0.8.4"\nedition = "2021"\n\n[dependencies]\nlib-plugin-manifest = { path = "../lib-plugin-manifest" }\nreqwest = { version = "0.12", features = ["stream", "json"] }\ntokio = { version = "1", features = ["full"] }\nserde = { version = "1.0", features = ["derive"] }\nserde_json = "1.0"\nthiserror = "2"\nfutures = "0.3"\nurlencoding = "2.1"' > /build/lib-plugin-registry/Cargo.toml

# Copy adi-plugin-registry with standalone Cargo.toml
COPY crates/adi-plugin-registry-http/src /build/adi-plugin-registry/src
RUN echo '[package]\nname = "adi-plugin-registry"\nversion = "0.8.4"\nedition = "2021"\n\n[[bin]]\nname = "adi-plugin-registry"\npath = "src/main.rs"\n\n[dependencies]\nlib-plugin-registry = { path = "../lib-plugin-registry" }\nlib-plugin-manifest = { path = "../lib-plugin-manifest" }\naxum = { version = "0.7", features = ["multipart"] }\ntower = "0.4"\ntower-http = { version = "0.5", features = ["cors", "trace", "limit"] }\ntokio = { version = "1.0", features = ["full"] }\nanyhow = "1.0"\ntracing = "0.1"\ntracing-subscriber = { version = "0.3", features = ["env-filter"] }\nserde = { version = "1.0", features = ["derive"] }\nserde_json = "1.0"\nsha2 = "0.10"\nhex = "0.4"\nchrono = { version = "0.4", features = ["serde"] }\nflate2 = "1"\ntar = "0.4"\ntokio-util = { version = "0.7", features = ["io"] }\nsemver = "1"' > /build/adi-plugin-registry/Cargo.toml

# Build release binary
WORKDIR /build/adi-plugin-registry
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
COPY --from=builder /build/adi-plugin-registry/target/release/adi-plugin-registry /app/adi-plugin-registry

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
