# movie-recommendation-service

Rust/Axum recommendation API for the movie reservation platform demo.

The service is intentionally deterministic. It uses an in-memory movie catalog
seeded from the reservation demo data and returns recommendations that include
`movie_reservation_movie_id` so agent and MCP flows can map suggestions to the
reservation service catalog.

## Run

```sh
USE_DUMMY=true PORT=8082 cargo run
```

Defaults:

- `PORT=8082`
- `USE_DUMMY=true`
- no OpenTelemetry export unless `OTEL_EXPORTER_OTLP_ENDPOINT` is set

`USE_DUMMY=false` is rejected at startup because this slice has no external
catalog, persistence, or secret-backed provider.

## HTTP Contract

- `GET /health`
- `GET /ready`
- `GET /movies?limit=10`
- `GET /recommendations?limit=5&preference=sci-fi`

`limit` defaults to `10` for `/movies`, `5` for `/recommendations`, and is
clamped to a maximum of `20`.

Health response:

```json
{
  "status": "ok",
  "service_name": "movie-recommendation-service"
}
```

Readiness response:

```json
{
  "status": "ready",
  "service_name": "movie-recommendation-service"
}
```

Recommendation response:

```json
{
  "recommendations": [
    {
      "id": "recommendation-the-matrix",
      "title": "The Matrix",
      "reason": "Matches demo preference 'sci-fi' using rating, runtime, and screening availability hints",
      "confidence": 0.99,
      "movie_reservation_movie_id": "44444444-4444-4444-8444-444444444435"
    }
  ]
}
```

Responses include `x-correlation-id` and `x-request-id`. Incoming
`x-correlation-id`, `x-request-id`, and W3C `traceparent` are propagated into
structured logs and tracing context when provided.

## Demo Faults

Faults are only applied to `GET /recommendations`.

Use `X-Demo-Fault` first:

```sh
curl -H 'X-Demo-Fault: slow-recommendation' \
  'http://127.0.0.1:8082/recommendations?limit=2'
```

Supported values:

- `none`
- `slow-recommendation`
- `recommendation-error`

If the header is absent, `DEMO_FAULT_MODE` is used as a fallback. Unknown fault
values are treated as `none`.

`recommendation-error` returns HTTP 503:

```json
{
  "error": {
    "code": "recommendation_unavailable",
    "message": "Recommendation service unavailable for demo fault",
    "fault": "recommendation-error"
  }
}
```

Internal failures return HTTP 500 with `error.code=internal_error`.

## Container Contract

The deployable artifact is a Linux container image for this repository. Infra
must consume an immutable image reference pinned by digest, for example:

```text
ghcr.io/movie-reservation-platform-lab/movie-recommendation-service@sha256:<digest>
```

Runtime expectations:

- process binds `0.0.0.0:${PORT:-8082}`
- container exposes `8082`
- liveness endpoint: `GET /health`
- readiness/container health endpoint: `GET /ready`
- no secrets are required for the current deterministic provider
- optional OTLP HTTP endpoint via `OTEL_EXPORTER_OTLP_ENDPOINT`

Build locally:

```sh
docker build -t movie-recommendation-service:local .
docker run --rm -p 8082:8082 movie-recommendation-service:local
```

## Checks

```sh
cargo fmt --check
cargo test
cargo check
```
