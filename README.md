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
| `SERVICE_VERSION` | Cargo package version | Bounded artifact identity in audit events and OTel resources |
| `DEPLOYMENT_ENVIRONMENT` | `local` | Bounded environment identity in audit events and OTel resources |
| `DEMO_AUTH_ENABLED` | `false` | Enables the isolated demo credential check; no existing route is protected |
| `DEMO_AUTH_USERNAME`, `DEMO_AUTH_PASSWORD` | unset | Both nonblank and required when demo auth is enabled; never logged |

Invalid ports, booleans, provider settings, and OTLP base URIs fail startup
with a diagnostic. Demo credentials are secrets: inject them at runtime and do
not put them in image layers, committed files, or command-line arguments.
Exporter endpoints are not written to application request logs.

## Authentication audit demo

The opt-in `POST /demo/auth/login` emits OCSF Authentication events as single-line
`{"audit":EVENT}` stdout records. FireLens routes those records to Firehose and
S3; Athena queries the archive. This service has no AWS publishing dependency.
The credential check creates no session or token. See the
[local demo and correlation guide](docs/audit-demo.md) for commands, outcomes,
failure behavior, and the difference between OTel and AWS request IDs.

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

Pushes to `main` publish a Linux AMD64 candidate to GHCR as `sha-<commit>-run-<run-id>-attempt-<attempt>`.
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

### Container security evidence

The pinned organization-owned actions publish the signed
`recommendation-service-security-evidence-<run-id>-attempt-<attempt>` artifact:
`component-candidate-evidence-v1alpha3.json`, verified image provenance,
CycloneDX SBOM, and subject-bound vulnerability report. Evidence is retained
for 14 days. Missing provenance, unavailable central policy, or unapproved CRITICAL findings
block the canonical evidence package. V1alpha3 evaluates the complete report
against current centrally approved exemptions; this repository supplies no
exemptions or ignore list. HIGH findings remain visible for admission review.
Rejected publication retains scan/policy diagnostics in a separate artifact.

Run/attempt tags are discovery hints, not deployment selectors. Environment
verification independently checks the successful canonical run and signed
package before admitting its exact digest to ECR. This producer has no AWS
credentials or deployment authority. Older runs without this package are not
eligible for the new admission path; use a fresh successful main run.
See [the shared action contract](https://github.com/movie-reservation-platform-lab/movie-platform-actions/blob/036531133bcefd454b5afc0eb55f8ba0328901ea/docs/container-candidate-actions.md).

This producer adopts [actions PR #18](https://github.com/movie-reservation-platform-lab/movie-platform-actions/pull/18)
at `036531133bcefd454b5afc0eb55f8ba0328901ea` for both publisher actions and the
PR/local scanner. Prepare receives `github-token: ${{ github.token }}` for its
authenticated canonical-main lookup, using the publishing job's existing
`contents: read` permission. The job's other permissions remain required;
passing its token does not reduce that token's authority. This release also
hardens evidence failure paths (including bounded legacy report reads and safe
errors) and scanner cleanup. Evidence remains v1alpha3.

Offline caller tests verify wiring and guards. Hosted PR scanning does not
exercise prepare or prove private-repository access or canonical publication;
those require separate live rollout acceptance. Publication remains restricted
to push events on this repository's canonical main. Rollback reverts both action
pins, the PR tooling checkout, and the documented local tooling pin to
`bb40579c285df0b581c48b10f9b34574d5c78639`, and removes the new prepare token input
together. See the [adoption plan](docs/plans/authenticated-prepare-adoption.md).


### Production-image checks before merge

The production image keeps the Rust release binary, curl readiness probe, TLS
certificates and tini on Debian Trixie. The build refreshes preinstalled runtime
packages as well as installing dependencies, so available Debian security fixes
are applied even when the base image has not yet been republished.

`container-security-check` builds the Rust `runtime` target for linux/amd64 on
PRs and manual runs, then uses the same reviewed v1alpha3 policy tooling as
publication. It has only `contents: read` permission. The entire scan directory
is uploaded even when the gate fails, as
`recommendation-service-pr-vulnerability-report-<run-id>-attempt-<attempt>`
(retention: 14 days). Complete findings, including all severities and unfixed
packages, remain available for remediation; failed/incomplete evaluation keeps
the check red. These reports are diagnostics, not signed candidate evidence.
Canonical main pushes use the publication job's exact-digest scan instead.

To reproduce locally with Node 24, Docker and a GitHub token available to the
shared tool as `GH_TOKEN`:

```sh
docker build --platform linux/amd64 --target runtime --tag movie-recommendation-service:pr-security .
bash automation/container-smoke.sh movie-recommendation-service:pr-security
# Use movie-platform-actions checked out at 036531133bcefd454b5afc0eb55f8ba0328901ea.
node ../movie-platform-actions/local-tools/container-security/lib/scan.mjs \
  movie-recommendation-service:pr-security \
  --evidence-version v1alpha3 --component recommendation-service \
  --output-dir /tmp/recommendation-service-pr-security
```

The scanner returns 0 for a policy pass, 1 for rejection, and 2 for an incomplete
check. Full reports survive rejection; an incomplete check retains partial
output for diagnosis. Local results do not replace a fresh successful canonical
publication or environment-owned verification/admission.
