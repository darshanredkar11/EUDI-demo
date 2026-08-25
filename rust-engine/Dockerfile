# Rust deterministic engine — multi-stage build.
FROM rust:1.97-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY tests ./tests
COPY config ./config
RUN cargo build --release --bin engine

FROM debian:stable-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/engine /usr/local/bin/engine
COPY config ./config
ENV CONFIG_DIR=/app/config PORT=8081
EXPOSE 8081
CMD ["engine"]
