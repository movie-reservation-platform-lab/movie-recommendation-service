# movie-recommendation-service

Rust/Axum recommendation API for the movie reservation platform demo.

This service is derived from
`/home/patex1987/development/axum_tools_random_api`, branch
`demo-multi-service-observability`, at commit `3ea72f3`. That commit remains in
this repository's Git history as the proven multi-service observability
baseline.

The service is intentionally deterministic. It uses an in-memory movie catalog
seeded from the reservation demo data and returns recommendations that include
`movie_reservation_movie_id` so agent and MCP flows can map suggestions to the
reservation service catalog.

## Run

```sh
USE_DUMMY=true PORT=8082 cargo run
```

Runtime configuration is parsed once at startup:

| Variable | Default | Contract |
| --- | --- | --- |
| `PORT` | `8082` | Integer from 1 through 65535 |
| `USE_DUMMY` | `true` | Must be `true`; no external provider exists |
| `DEMO_FAULT_MODE` | `none` | Allowlisted recommendation fault fallback |
| `ALLOW_REQUEST_DEMO_FAULTS` | `false` | Enables the allowlisted `X-Demo-Fault` header when `true` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | unset | Validated HTTP(S) base URI for OTLP HTTP trace/metric export |
| `OTEL_RESOURCE_ATTRIBUTES` | SDK defaults | Platform-owned resource attributes such as deployment environment |
| `RUST_LOG` | `info` | `tracing-subscriber` filter |

Invalid ports, booleans, provider settings, and OTLP base URIs fail startup
with a diagnostic. No current setting is a secret, and exporter endpoints are
not written to application request logs.

## HTTP Contract

- `GET /health`
- `GET /ready`
- `GET /movies?limit=10`
- `GET /recommendations?limit=5&preference=sci-fi`

`limit` defaults to `10` for `/movies`, `5` for `/recommendations`, and is
clamped to a maximum of `20`. Zero is valid. `preference` is optional, trimmed,
and limited to 128 bytes.

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

Responses include `x-correlation-id` and `x-request-id`. Non-empty incoming
values up to 128 bytes are echoed; absent, invalid, or oversized values are
replaced with bounded service-generated IDs. Valid W3C `traceparent` context is
used as the parent trace and in structured request logs. Malformed or all-zero
trace/span IDs are rejected by the W3C propagator.

Malformed query values return HTTP 400 with a JSON error envelope. An oversized
preference uses `error.code=invalid_preference`; other deserialization failures
use `error.code=invalid_query`. Internal failures return HTTP 500 with
`error.code=internal_error` and do not expose provider diagnostics.

## Demo Faults

Faults are only applied to `GET /recommendations`. Request-controlled faults
are disabled by default; explicitly enable them in a demo environment:

```sh
ALLOW_REQUEST_DEMO_FAULTS=true USE_DUMMY=true PORT=8082 cargo run
```

Then use `X-Demo-Fault`:

```sh
curl -H 'X-Demo-Fault: slow-recommendation' \
  'http://127.0.0.1:8082/recommendations?limit=2'
```

Supported values:

- `none`
- `slow-recommendation`
- `recommendation-error`

When request faults are enabled, a present header takes precedence over
`DEMO_FAULT_MODE`, including an unknown value being treated as `none`. When the
gate is disabled, the header is ignored and the typed startup fallback is used.
Faults never apply to health, readiness, or movie listing.

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

`slow-recommendation` waits asynchronously for two seconds and then follows the
normal success path.

## Observability and Lifecycle

Application lifecycle and completed-request records are emitted as one JSON
object per line. Request records contain bounded `service_name`, `event`,
`trace_id`, `correlation_id`, `request_id`, `fault`, `http_route`, `http_status`,
and `duration_ms` fields. Request workers enqueue these records without waiting
on stdout. The bounded 1,024-record queue drops excess events rather than
blocking HTTP work and reports the aggregate drop count during shutdown.

OpenTelemetry export is optional. Exporter construction or export failure does
not make health/readiness fail. Request metric attributes are limited to static
route, HTTP status, boolean preference presence, and allowlisted fault values;
request, trace, user, and movie IDs are not metric labels.

The process handles Ctrl-C and Unix SIGTERM through Axum graceful shutdown.
Trace and metric providers receive a shared five-second shutdown budget; the
process continues terminating if an exporter does not finish in time.

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
- the image health check resolves `${PORT:-8082}`
- no secrets are required for the current deterministic provider
- optional OTLP HTTP endpoint via `OTEL_EXPORTER_OTLP_ENDPOINT`
- request-controlled demo faults are off unless
  `ALLOW_REQUEST_DEMO_FAULTS=true` is explicitly configured
- SIGTERM initiates graceful HTTP shutdown and bounded telemetry flush

Build locally:

```sh
docker build -t movie-recommendation-service:local .
docker run --rm -p 8082:8082 movie-recommendation-service:local
```

Publishing is owned by CI for this repository: build and test once, publish one
candidate image, and record its source revision and registry digest. Platform
repositories promote the same digest; they must not rebuild this source or rely
on a mutable tag. This repository does not own AWS resources or environment
selection.

Pushes to `main` publish a Linux AMD64 candidate to GHCR as `sha-<commit>`.
CI disables BuildKit's automatic registry attestation to preserve the
single-image manifest required by the first environment admission slice, then
records explicit GitHub build provenance against the published digest.

The baseline commit is reachable on the
`demo-multi-service-observability` branch. Verify locally with:

```sh
git branch --all --contains 3ea72f3
```

## Checks

```sh
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
node --test automation/tests/*.test.mjs
```

When Docker is available, also run `docker build --check .` and a local image
build, followed by:

```sh
bash automation/container-smoke.sh movie-recommendation-service:local
```

These commands verify an artifact only; they do not publish or deploy it.
