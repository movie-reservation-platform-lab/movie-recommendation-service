# Implementation Plan: Stabilize the Recommendation API

## 1. Summary

Issue #1 is a design-review-level stabilization of the existing Rust/Axum
service, not a greenfield rewrite. The current branch already contains the
functional recommendation baseline, so the recommended approach is to preserve
its public HTTP and demo-fault behavior while hardening the boundaries that are
still incomplete: typed startup configuration, bounded request metadata,
standards-compliant trace-context extraction, stable client errors, graceful
container shutdown, bounded telemetry flushing, deterministic tests, and the
immutable image handoff documentation.

The work should be reviewed as three small PR-shaped slices even though this
implementation session will leave one uncommitted working tree, as requested:

1. HTTP contract, configuration, and deterministic tests.
2. Process lifecycle and observability hardening.
3. Container/dependency/documentation cleanup and release verification.

## 2. Goals

- Preserve `GET /health`, `GET /ready`, `GET /movies`, and
  `GET /recommendations` response behavior for valid requests.
- Preserve deterministic recommendation ranking and reservation-movie IDs from
  the proven demo baseline.
- Preserve allowlisted `none`, `slow-recommendation`, and
  `recommendation-error` behavior. Keep the typed startup fallback while
  requiring an explicit startup gate before a request header can select a
  fault; when enabled, the header takes precedence over the fallback.
- Make runtime configuration explicit, validated once at startup, and free of
  request-path environment reads or panics.
- Bound caller-controlled preference and correlation metadata before using it
  for work, logs, spans, or response headers.
- Use the W3C propagator as the source of truth for valid `traceparent` data.
- Return stable, safe JSON errors for malformed or oversized query input and
  internal failures.
- Handle both Ctrl-C and Unix termination signals and bound telemetry shutdown
  so container termination cannot hang indefinitely.
- Replace real-time delay tests with paused Tokio time and cover the normal,
  bounded, malformed, slow, failure, propagation, configuration, and lifecycle
  paths required by issue #1.
- Keep the output image contract independent of AWS or environment-specific
  deployment ownership.

## 3. Non-goals

- No MCP tools, agent orchestration, or Python package in this repository.
- No AWS resources, deployment manifests, environment promotion, or image push.
- No authentication, persistence, external movie provider, cache, or live
  reservation-service dependency.
- No recommendation algorithm redesign or machine-learning behavior.
- No new API version, route, schema registry, Pact broker, or speculative
  service abstraction.
- No Git history rewrite to make `3ea72f3` an ancestor of the current branch.
- No commits or pull requests during this implementation session.

## 4. Current State

- Issue #1 defines the stabilization scope and requires provenance, stable
  contracts, explicit configuration, focused tests, and an immutable publishing
  contract.
- The current branch is `issue-1-stabilize-recommendation-api`, one commit ahead
  of `origin/main`. Existing uncommitted README and generated AI-guidance files
  belong to the user and must be preserved.
- `3ea72f3` is reachable from the local and remote
  `demo-multi-service-observability` branch, but is not an ancestor of the
  current branch. Its Rust baseline is nevertheless present almost verbatim in
  the current implementation. Provenance should be documented, not repaired by
  rewriting history.
- `src/main.rs` currently owns startup, routes, request parsing, trace/correlation
  parsing, demo faults, response mapping, logging, shutdown, and most tests. It
  reads `DEMO_FAULT_MODE` on every recommendation request and uses `unwrap()` or
  `panic!` for bind, serve, `PORT`, and unsupported provider configuration.
- `src/di/movie_service.rs` reads `USE_DUMMY` itself and panics unless it is
  `true`.
- `src/services/movie/dummy.rs` provides the deterministic ten-movie catalog and
  ranking behavior. Its ordinary recommendation behavior already satisfies the
  functional baseline and should not be changed.
- `src/telemetry.rs` provides optional OTLP traces/metrics and bounded metric
  dimensions, but uses text-formatted tracing output and performs synchronous,
  unbounded provider shutdown.
- `Dockerfile` builds a non-root runtime image with `/ready` as its health check,
  but the health command is hard-coded to port `8082` even when `PORT` is
  overridden.
- The current 11 tests pass, but the slow-path test takes two real seconds. There
  are no tests for defaults/clamps, malformed input, oversized preference,
  service failures, correlation bounds, full W3C propagation validation,
  startup configuration, SIGTERM selection, or bounded telemetry attributes.
- `reqwest` and `rand` are direct dependencies but are unused by this service.
- Initial verification before implementation passed format, check, Clippy with
  warnings denied, and all tests.

The local Programming KB reinforces two design constraints:

- `API Compatibility` recommends naming the exact wire and semantic behavior
  being preserved and using executable provider fixtures before adding heavier
  cross-repository contract infrastructure.
- `Independent Deployability` and
  `Prefer Compatibility-First Independent Deployments` distinguish a buildable
  container from a compatible, independently promotable artifact and keep the
  prior artifact usable through rollout and rollback.
- `Multi-Service Release Composition` assigns image promotion and environment
  digest selection to the platform workflow, while the service repository owns
  build/test evidence and an immutable candidate contract.

## 5. Requirements and Assumptions

### Confirmed Requirements

- Pin `axum_tools_random_api@3ea72f3` as the implementation provenance.
- Stabilize the standalone Axum recommendation API without restoring the
  historical co-located MCP package.
- Preserve health/readiness, movies, recommendations, controlled faults,
  configuration, context propagation, and observability behavior.
- Add focused tests for normal, bounded input, slow, failure, propagation, and
  lifecycle behavior.
- Pass format, compile, Clippy, tests, and release build from the repository
  root.
- Document the immutable container publishing and consumption contract.
- Do not commit, push, publish, deploy, or mutate shared environment state.

### Assumptions

- The README contract and `3ea72f3` behavior are the compatibility target for
  issue #1 because the MCP consumer repository is outside this workspace.
- Valid existing requests and successful/error response shapes are stable.
  Malformed-query responses were not stable; converting Axum's default text
  rejection into a safe JSON error is an intentional stabilization.
- A preference longer than 128 bytes is invalid and returns HTTP 400. Empty or
  whitespace-only preferences retain their current "not present" semantics.
- `limit=0` remains valid, omitted limits retain route-specific defaults, and
  values above 20 remain clamped to 20.
- Correlation and request IDs longer than 128 bytes are not propagated; a local
  bounded ID is generated instead. Valid bounded values are echoed unchanged.
- An unknown demo fault remains `none`. `X-Demo-Fault` is ignored unless
  `ALLOW_REQUEST_DEMO_FAULTS=true`; once enabled, a present header overrides
  the configured environment fallback, including when its value is unknown.
- OTLP exporter setup failure disables that signal and leaves local structured
  logs operational. Exporter failure is not a service-readiness failure.
- Telemetry receives at most five seconds to shut down. Expiry is logged and
  process termination continues.
- Repository-owned provider tests are sufficient for this small contract. A
  cross-repository consumer-driven contract can be added later if the consumer
  inventory grows.

### Open Questions

- The exact deployed MCP consumer versions are not available in this repository,
  so this plan cannot prove pairwise compatibility with an active environment.
  The implementation will preserve documented fixtures and leave cross-repo
  verification to the platform/consumer workflow.
- The image registry workflow is not present here. This issue will document the
  output contract and verify a release build/Dockerfile; publishing remains an
  explicitly separate action.

Neither question blocks the repository-local implementation requested by the
issue.

## 6. Proposed Design

### Configuration boundary

Add a small `config` module that parses `PORT`, `USE_DUMMY`,
`DEMO_FAULT_MODE`, `ALLOW_REQUEST_DEMO_FAULTS`, and
`OTEL_EXPORTER_OTLP_ENDPOINT` once. `AppConfig::from_env` returns a diagnostic
`Result` for invalid ports, booleans, provider values, and OTLP base URIs. The
handler state receives the typed provider, default fault, and request-fault
gate and never reads process environment variables.

No configuration values are secret. The OTLP endpoint must be an absolute
HTTP(S) base URI without credentials or a query string and must never be
emitted in logs. Platform-owned resource attributes continue to use the
standard `OTEL_RESOURCE_ATTRIBUTES` input.

### HTTP boundary

Move router/handler ownership into `src/http/` while keeping the domain service
independent of Axum:

- `src/http/mod.rs`: typed queries/responses, router construction, handlers,
  safe error mapping, limits, and in-process handler tests.
- `src/http/context.rs`: bounded correlation/request metadata and W3C parent
  extraction using `TraceContextPropagator`.
- `src/demo_fault.rs`: the allowlisted fault enum and precedence selection used
  by configuration and HTTP code.

Handlers construct request context even when query extraction fails so every
response retains bounded correlation headers and telemetry. `Result<Query<_>,
QueryRejection>` maps malformed input to the stable JSON envelope. Preference
length is checked before ranking.

### Domain/service boundary

Keep the existing `AsyncMovieService` because it is already the real injectable
boundary used to test safe internal failures. Keep the deterministic catalog and
ranking implementation unchanged except for tests or names needed by the module
move. Configure the provider outside `di`; do not let the provider constructor
read environment variables or panic.

### Trace, log, and metric behavior

Use one extracted OpenTelemetry `Context` for the inbound span and the public
log correlation trace ID. Invalid or all-zero W3C IDs are rejected by the SDK
propagator and replaced with a locally generated bounded trace ID.

Emit explicit JSON application lifecycle/request events through a bounded
background writer while retaining the existing `tracing-subscriber` and
OpenTelemetry span layers. Request paths use a non-blocking enqueue; when the
1,024-event buffer is full or contended, the event is dropped and the aggregate
count is reported at shutdown. This avoids a new runtime dependency and stdout
I/O on Tokio workers while preserving the documented service, event, trace,
correlation, request, fault, route, status, and duration fields. Keep metric
labels limited to static routes, numeric status, boolean presence, and the
allowlisted fault enum.

### Lifecycle

Make startup a fallible `run` path: configuration, telemetry initialization,
listener bind, and server errors return diagnostics instead of panicking. The
graceful-shutdown future listens for Ctrl-C and, on Unix, SIGTERM. After Axum
stops (including a server error), telemetry providers are shut down on bounded
worker threads so exporter behavior cannot indefinitely block process exit.

### Artifact contract

Keep the multi-stage, non-root container. Make the health check honor
`${PORT:-8082}` and document that CI publishes one image candidate and platform
configuration consumes the exact registry digest. Do not add platform-specific
deployment files.

## 7. Alternatives Considered

### Alternative A: Patch only missing tests in `main.rs`

- Pros: Smallest textual diff; very low refactor risk.
- Cons: Leaves environment reads, panic paths, weak trace parsing, SIGTERM gap,
  unbounded shutdown, and a 700-line mixed-responsibility entrypoint.
- Decision: Rejected because tests alone would not meet the explicit runtime,
  trust-boundary, lifecycle, or operability acceptance criteria.

### Alternative B: Focused boundary extraction and hardening

- Pros: Preserves the proven domain behavior, makes each trust/lifecycle
  boundary independently testable, and creates three human-reviewable slices
  without adding dependencies or speculative layers.
- Cons: Moves code and therefore creates a larger diff than isolated patches.
- Decision: Recommended. Restrict extraction to configuration, HTTP/context,
  and shared fault types; retain the existing domain/service structure.

### Alternative C: Introduce a library crate, middleware stack, generic service
layers, and external contract tooling

- Pros: Could support black-box integration tests and future consumers.
- Cons: Adds public internal APIs, abstraction, dependency, and operational
  overhead before there is evidence of multiple implementations or consumers.
- Decision: Rejected for this intentionally small service. Reconsider Pact or a
  public library boundary only when cross-repository compatibility evidence can
  no longer be maintained with fixtures.

## 8. API / Interface Changes

Valid-request behavior remains:

- `GET /health` -> HTTP 200 with `status=ok`.
- `GET /ready` -> HTTP 200 with `status=ready`.
- `GET /movies?limit=<usize>` -> HTTP 200 JSON array, default 10, maximum 20.
- `GET /recommendations?limit=<usize>&preference=<text>` -> HTTP 200 envelope,
  default 5, maximum 20.
- With `ALLOW_REQUEST_DEMO_FAULTS=true`,
  `X-Demo-Fault: slow-recommendation` -> two-second delayed success.
- With `ALLOW_REQUEST_DEMO_FAULTS=true`,
  `X-Demo-Fault: recommendation-error` -> HTTP 503 with
  `recommendation_unavailable`.
- Internal movie-service failure -> HTTP 500 with `internal_error` and no
  provider diagnostic.
- Valid `x-correlation-id` and `x-request-id` values up to 128 bytes are echoed.

Newly stabilized invalid-input behavior:

- Malformed query values -> HTTP 400 JSON error with `code=invalid_query`.
- Preference over 128 bytes -> HTTP 400 JSON error with
  `code=invalid_preference`.
- Empty/invalid/oversized correlation IDs -> generated bounded response IDs.
- Invalid W3C parent context -> new local trace context; it is never reported as
  the inbound trace ID.

Internal Rust interfaces change as follows:

- `AppConfig` owns validated runtime settings.
- `build_app` accepts the movie service, telemetry, typed default fault, and
  request-fault gate.
- `create_movie_service` accepts a typed provider and no longer reads
  environment variables.
- Telemetry initialization is fallible and shutdown accepts a finite timeout.

## 9. Data Model / Persistence Changes

None. The movie and recommendation JSON fields remain unchanged. There is no
schema migration, backfill, stored state, or data rollback.

## 10. Security, Privacy, and Abuse Considerations

- Bound preference, correlation ID, and request ID before downstream work or
  logging to prevent oversized allocations/log amplification.
- Accept only the three fault modes; do not expose arbitrary delay durations or
  error injection.
- Disable request-triggered faults by default and require the privileged
  startup configuration `ALLOW_REQUEST_DEMO_FAULTS=true` before honoring the
  header.
- Never log the OTLP endpoint, environment contents, full request headers,
  query preference, or internal service errors.
- Keep error responses static so provider/runtime diagnostics do not cross the
  HTTP boundary.
- Validate trace context with the W3C propagator, including non-zero trace and
  parent span identifiers, before treating it as correlation evidence.
- Continue running as a non-root container user. No secrets are needed by the
  deterministic provider.
- Auth and rate limiting remain deployment-boundary concerns for this internal
  demo API; external internet exposure is out of scope and should not be inferred
  from this plan.

## 11. Performance, Scalability, and Reliability Considerations

- Recommendation ranking remains bounded by the ten-item in-memory catalog and
  the public result maximum of 20.
- Slow faults use asynchronous Tokio time and do not block a runtime worker;
  they require explicit startup authorization, limiting accidental exposure.
- Environment is parsed once, avoiding per-request process-state access.
- No request-controlled value becomes a metric dimension. Route and fault
  attributes are closed sets; status code has a finite HTTP range.
- Health/readiness never depend on fault injection or exporters.
- OTLP setup/export/shutdown failure degrades telemetry, not serving behavior.
- Bounded telemetry shutdown trades possible loss of final spans/metrics for a
  guaranteed container termination bound; a warning makes that loss visible.
- Structured application events use a bounded non-blocking queue and one
  background writer so slow stdout cannot stall request workers. Excess events
  are deliberately dropped and counted rather than expanding memory.
- There is no shared mutable catalog state or external retry loop.

## 12. Implementation Steps

### Suggested PR 1: Contract, configuration, and deterministic tests

1. Add typed configuration and demo-fault parsing.
   - Change: Parse defaults/valid values once; reject invalid port/provider
     settings without panics; preserve unknown-fault-as-none behavior; require
     an explicit request-fault gate; validate the OTLP base URI.
   - Files/modules likely affected: `src/config.rs`, `src/demo_fault.rs`,
     `src/di/movie_service.rs`, `src/main.rs`.
   - Notes: Test parsing through pure value inputs, not process-global env
     mutation.
   - Verification: Focused config and fault unit tests.

2. Extract and harden the HTTP boundary.
   - Change: Move router/handlers into `src/http/mod.rs`; add bounded request
     metadata, SDK W3C parsing, stable query errors, and preference validation.
   - Files/modules likely affected: `src/http/mod.rs`, `src/http/context.rs`,
     `src/main.rs`.
   - Notes: Preserve valid response schemas and fault precedence.
   - Verification: In-process Axum tests for every public route and error path.

3. Complete deterministic contract tests.
   - Change: Add table-driven defaults/clamps/fault/context tests, mock service
     failures, and paused-time slow-fault coverage.
   - Files/modules likely affected: `src/http/tests.rs`,
     `src/services/movie/dummy.rs`, `Cargo.toml`.
   - Notes: Add Tokio `test-util`; do not bind a real port or sleep in wall time.
   - Verification: `cargo test --all-targets` completes without a two-second
     wall-clock delay.

### Suggested PR 2: Lifecycle and observability

4. Make startup and shutdown fallible and container-aware.
   - Change: Return startup/bind/serve diagnostics; listen for Ctrl-C and
     SIGTERM; ensure telemetry shutdown runs after serve completion.
   - Files/modules likely affected: `src/main.rs`.
   - Notes: No `unwrap`/`expect` on runtime paths. Add a small injectable
     shutdown-future selector test rather than sending OS signals in unit tests.
   - Verification: Lifecycle unit tests, Clippy, and a local process smoke test
     when available.

5. Produce structured telemetry and bound exporter shutdown.
   - Change: Emit explicit JSON application events through a bounded,
     non-blocking background writer, retain OpenTelemetry spans and
     low-cardinality attributes, and time-bound provider shutdown.
   - Files/modules likely affected: `src/telemetry.rs`, `src/http/mod.rs`,
     `Cargo.toml`.
   - Notes: Shutdown providers independently so one stuck signal does not
     prevent attempting the other.
   - Verification: Attribute-construction unit test plus full suite; smoke logs
     contain the documented fields without logging raw preference/endpoints.

### Suggested PR 3: Artifact and handoff cleanup

6. Minimize the dependency and runtime artifact surface.
   - Change: Remove unused `reqwest` and `rand`; make direct Tokio features
     explicit; make Docker health checks honor configured `PORT`.
   - Files/modules likely affected: `Cargo.toml`, `Cargo.lock`, `Dockerfile`.
   - Notes: Do not add publishing credentials or mutable environment manifests.
   - Verification: `cargo check --all-targets`, release build, and Docker build
     or static Dockerfile verification when Docker is available.

7. Finalize provenance, configuration, contract, and publishing docs.
   - Change: Document environment variables, validation/failure semantics,
     query/metadata bounds, graceful shutdown, check commands, baseline branch,
     and digest-pinned platform consumption.
   - Files/modules likely affected: `README.md`, this plan.
   - Notes: Preserve the user's existing uncommitted provenance paragraph.
   - Verification: Every documented command and example matches the code.

8. Obtain independent review evidence and remediate findings.
   - Change: Ask repository review agents for file/line findings across system
     design, security, maintainability, and performance; fix in-scope findings.
   - Files/modules likely affected: as identified by review.
   - Notes: Findings must cite concrete file/line evidence. No speculative
     feature expansion.
   - Verification: Re-run the complete required command set after remediation.

## 13. Testing Strategy

### Pure unit tests

- `PORT`: absent, valid, zero rejection, non-numeric, overflow.
- `USE_DUMMY`: absent/true/case handling and explicit rejection of false or
  unknown values.
- `DEMO_FAULT_MODE`: every allowlisted value, whitespace/case, unknown fallback.
- `ALLOW_REQUEST_DEMO_FAULTS`: absent/false/true and invalid value rejection.
- `OTEL_EXPORTER_OTLP_ENDPOINT`: absent, valid HTTP(S) base URI, trailing slash,
  unsupported scheme, relative URI, credentials, and query rejection.
- Limit: route defaults, zero, maximum, and over-maximum clamp.
- Preference: absent, blank, 128-byte boundary, 129-byte rejection.
- Disabled request-fault gate ignores the header; enabled header precedence over
  startup default, including unknown header.
- W3C trace parent: valid, malformed field count/hex, all-zero trace ID,
  all-zero parent ID, invalid flags.
- Correlation/request metadata: bounded values preserved; empty/oversized values
  replaced.
- Metric attributes contain only the bounded route/status/fault dimensions.
- Application-event writing emits one JSON line and bounded-queue saturation
  increments the drop count without blocking.
- Shutdown selector completes when either supplied signal future resolves.

### In-process Axum tests

- Health and readiness status/body/correlation headers.
- Movies default and explicit limits plus reservation mapping.
- Recommendations default and explicit limits plus deterministic preference.
- Malformed limit returns stable safe HTTP 400 JSON.
- Oversized preference returns stable safe HTTP 400 JSON.
- `recommendation-error` returns the exact 503 envelope.
- `slow-recommendation` advances paused Tokio time by two seconds and succeeds
  without two seconds of wall-clock delay.
- Fault configuration does not affect health/readiness/movies.
- Injected movie-service errors return safe HTTP 500 bodies without exposing the
  diagnostic.
- Valid context headers are echoed and valid trace IDs are selected.

### Build/artifact checks

Run, in order:

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

When a Docker daemon is available:

```sh
docker build --check .
docker build -t movie-recommendation-service:local .
```

Optional process smoke verification should start the already-built binary with
OTLP disabled, call `/health`, `/ready`, and one normal/fault recommendation,
then send SIGTERM and confirm timely exit. This must remain local and must not
publish or deploy the image.

## 14. Rollout / Migration Plan

- No data or schema migration is required.
- Merge/review in the three proposed slices. Each slice must pass the full Rust
  checks independently.
- Build one candidate image in this repository and identify it by registry
  digest plus source revision. Do not rebuild it during environment promotion.
- Platform owners update only the recommendation-service digest and retain the
  previous digest through the rollback window.
- Before promotion, run provider fixtures and the platform-owned smoke path
  against the active MCP/agent versions. During rollout, old and new service
  tasks may coexist; both preserve the same valid-request wire contract.
- Observe readiness, HTTP error rate, request duration, controlled-fault counts,
  and shutdown duration. Exporter failure must not remove readiness.
- Rollback is selection of the previous exact image digest. Because there is no
  persisted state or changed downstream contract, no data rollback is needed.
- If newly stable invalid-query JSON conflicts with an unknown consumer, revert
  PR slice 1 while retaining lifecycle/artifact hardening; valid requests are
  unaffected.

## 15. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---:|---:|---|
| Refactoring handlers changes a valid wire response | High | Medium | Preserve fixtures and add in-process tests before/with the move |
| Oversized input or IDs amplify work/log volume | Medium | Medium | Enforce 128-byte bounds and avoid raw value metric labels |
| Invalid `traceparent` is logged as trusted correlation | Medium | Medium | Use the SDK W3C propagator and test zero/malformed identifiers |
| SIGTERM bypasses graceful shutdown in the container | High | High in container stop | Listen for Unix terminate as well as Ctrl-C |
| Exporter shutdown hangs process termination | High | Low/Medium | Shut down providers on independent workers with one finite deadline |
| Bounded shutdown drops final telemetry | Medium | Low | Use a five-second budget and emit a visible warning on expiry |
| New invalid-query JSON surprises an undocumented consumer | Medium | Low | Limit change to invalid requests, document it, keep slice independently revertible |
| Provenance wording implies current-branch ancestry | Low | Medium | State that `3ea72f3` is reachable on the baseline branch and was adopted |
| Docker health check fails when `PORT` is overridden | Medium | High today | Resolve the health URL from `${PORT:-8082}` |
| Unauthenticated request fault causes avoidable delay/errors | High | Medium | Ignore `X-Demo-Fault` by default; require explicit startup authorization and keep values allowlisted |
| Slow stdout blocks Tokio request workers | High | Medium | Use a bounded non-blocking queue and one background writer; count dropped events |
| Large mixed implementation is difficult to review | Medium | High | Organize changes and handoff notes into three PR-shaped slices; do not commit here |

## 16. Done Criteria

- The four documented routes preserve their valid-request status and JSON
  contracts.
- Limit defaults/clamps and the 128-byte preference bound are explicit and
  tested.
- Controlled fault allowlisting, secure default gate, enabled precedence,
  two-second delay, and 503 response are preserved and tested deterministically.
- Correlation/request metadata is bounded, valid values are echoed, malformed
  W3C trace context is rejected, and propagation tests pass.
- Runtime paths contain no input/config/bind/serve `unwrap`, `expect`, or panic.
- Configuration is parsed once and unsupported provider, boolean, port, or OTLP
  configuration exits with a safe diagnostic.
- Ctrl-C and SIGTERM trigger graceful Axum shutdown.
- Telemetry output is structured and buffered away from request workers, metric
  attributes remain bounded, and logger/provider shutdown share a finite
  deadline.
- Unused network/random dependencies are removed.
- Docker health uses the configured port and the runtime remains non-root.
- README pins `3ea72f3`, lists all runtime configuration and checks, and assigns
  digest promotion/deployment to platform repositories.
- Format, check, Clippy with denied warnings, all-target tests, and release build
  pass.
- Review agents report no unresolved in-scope high/medium findings with
  file/line evidence.
- No commit, push, image publication, deployment, or AWS mutation occurred.

## 17. Review Checklist

- [x] Requirements are explicit
- [x] Non-goals are explicit
- [x] Existing code conventions were checked
- [x] Alternatives were considered
- [x] Security implications were reviewed
- [x] Scalability and reliability implications were reviewed
- [x] Testing strategy is complete
- [x] Rollout and rollback are defined
- [x] Implementation steps are ordered and concrete

## 18. Handoff Prompt for Implementation Agent

```text
Implement the plan in docs/plans/stabilize-recommendation-api.md.

Constraints:
- Stay within issue #1 and the three suggested PR-shaped slices.
- Do not introduce new dependencies; Tokio test-util is a feature change to an
  existing dependency.
- Preserve valid public HTTP behavior, deterministic recommendation ranking,
  and the exact controlled fault contract.
- Preserve user-owned uncommitted changes and generated AI guidance.
- Do not add MCP code, platform deployment, AWS resources, secrets, publishing,
  commits, or pushes.
- If implementation reality changes a valid public contract or requires a new
  dependency, stop and update the plan before proceeding.

Relevant files/modules:
- src/main.rs
- src/config.rs
- src/demo_fault.rs
- src/http/mod.rs
- src/http/context.rs
- src/http/tests.rs
- src/di/movie_service.rs
- src/services/movie/**
- src/telemetry.rs
- Cargo.toml
- Cargo.lock
- Dockerfile
- README.md

Expected verification commands:
- cargo fmt --all -- --check
- cargo check --all-targets
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test --all-targets
- cargo build --release
- docker build --check . (when Docker is available)
```

## 19. Implementation Verification Record

Implemented on the current `issue-1-stabilize-recommendation-api` branch as one
uncommitted working tree organized by the three PR-shaped slices above. On
2026-08-10, the following checks passed after review remediation:

- `cargo fmt --all -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets` (`40 passed; 0 failed`)
- `cargo build --release`
- `git diff --check`
- `docker build --check .`
- `docker build -t movie-recommendation-service:codex-check .`

Local container smoke verification confirmed the custom-port health/readiness
contract, request-fault headers ignored by default, the exact enabled 503 fault
envelope, a successful enabled slow fault after approximately two seconds,
structured application events, a non-root image user, the configured-port
health check, and graceful SIGTERM shutdown with exit code zero.

System-design, security, maintainability, and performance reviewers re-audited
the remediated diff and reported no unresolved high- or medium-severity
findings. No commit, push, image publication, deployment, or shared-environment
mutation was performed.
