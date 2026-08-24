# Project AI Guidance

This repository owns the standalone Rust/Axum movie recommendation API. It
provides a small stable HTTP contract for the recommendation MCP, controlled
slow/failure scenarios for the demo, and OpenTelemetry traces, metrics, and
structured logs.

## Implementation Provenance

- This repository is derived from
  `/home/patex1987/development/axum_tools_random_api`, branch
  `demo-multi-service-observability`, at commit `3ea72f3`.
- Commit `3ea72f3` is present in this repository's Git history and is the proven
  multi-service observability baseline.
- Preserve the adopted Axum/Tokio, request-context, OpenTelemetry, controlled
  fault, in-process test, lifecycle, and container behavior while stabilizing
  it as the independently deployable recommendation service.
- The historical branch also contained `axum-tools-mcp/`; MCP ownership now
  belongs to `movie-recommendation-mcp`, not this service repository.

## Repository Layout

- `src/main.rs`: HTTP routes, request mapping, recommendation behavior, demo
  fault selection, and process lifecycle.
- `src/telemetry.rs`: OpenTelemetry providers, structured tracing, and bounded
  application metrics.
- `src/recommendation/`: recommendation-domain behavior where extracted.
- `src/http/`: HTTP-specific helpers where extracted.
- `Dockerfile`: immutable service artifact and health-check contract.
- `.ai/`: canonical AI guidance, skills, and read-only review agents.

## Development Commands

- Format: `cargo fmt --all -- --check`
- Compile: `cargo check --all-targets`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Tests: `cargo test --all-targets`
- Build: `cargo build --release`

Run focused tests while iterating, then format, Clippy, and the full test suite
before handoff.

## Rust And Axum Rules

- Prefer explicit enums and typed request/response structs over stringly typed
  internal behavior.
- Keep Axum extractors, headers, status codes, and response mapping at the HTTP
  boundary.
- Keep recommendation rules deterministic and testable without starting a
  network listener.
- Keep async sections non-blocking; use finite timeouts and explicit shutdown
  behavior for external I/O when introduced.
- Avoid `unwrap()` and `expect()` on request/runtime paths unless an invariant is
  both local and proved.
- Do not introduce traits or layers until they remove real coupling in this
  intentionally small service.

## Public And Observability Contracts

- Preserve `/health`, `/ready`, `/movies`, and `/recommendations` behavior unless
  an explicit compatibility plan changes it.
- Preserve controlled `slow-recommendation` and `recommendation-error` demo
  behavior without allowing arbitrary fault injection.
- Preserve W3C trace context and bounded correlation/request metadata.
- Keep IDs, free text, and unbounded values out of metric labels.
- Keep delivery/DORA telemetry outside service request telemetry.
- Keep service shutdown flushing telemetry without hanging indefinitely.

## Repository Boundaries

- This service owns recommendation behavior, not MCP tool contracts or agent
  orchestration.
- Publish an immutable container image. Environment selection and AWS deployment
  belong to the platform repositories.
- Do not add or mutate AWS resources from this repository.

## Testing Guidance

- Test handlers through an in-process Axum router rather than a real port when
  possible.
- Test recommendation rules as ordinary Rust functions/modules.
- Cover health/readiness, bounds/defaults, malformed input, slow/error faults,
  trace/correlation parsing, and safe error responses.
- Use paused Tokio time for deterministic delay/timeout behavior when supported.

## Safety

- Do not commit secrets, tokens, local env values, production payloads, or
  generated telemetry data.
- Treat configurable endpoints/exporters, request headers, logs, and provider
  errors as trust boundaries.
- Do not push, deploy, promote, or mutate shared environment state without
  explicit user instruction.

## Planning And Review

- Use `principal-engineer-planner` before public-contract, concurrency,
  observability, dependency, or deployment-artifact changes.
- Use the pinned baseline as the starting point; do not replace it with an
  unrelated Axum scaffold.
- Save implementation plans under `docs/plans/`.
- Ask review agents for findings first and require file/line evidence.
