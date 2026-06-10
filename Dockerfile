# syntax=docker/dockerfile:1.7

FROM rust:1-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock /app/
COPY src /app/src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/axum_movies_random_tools_api /tmp/axum-tools-random-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl tini \
 && rm -rf /var/lib/apt/lists/*

RUN useradd -r -u 10001 appuser
COPY --from=build /tmp/axum-tools-random-api /usr/local/bin/axum-tools-random-api
USER appuser

EXPOSE 8082
HEALTHCHECK --interval=30s --timeout=3s --retries=3 CMD \
  sh -c 'curl -fsS http://127.0.0.1:8082/health || exit 1'

ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["axum-tools-random-api"]
