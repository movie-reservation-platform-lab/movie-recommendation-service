---
name: rust-axum-service
description: Use when implementing, refactoring, reviewing, or explaining this Rust/Axum HTTP service, including typed request and response contracts, extractors, Tokio async behavior, error handling, configuration, OpenTelemetry integration, controlled fault injection, and graceful shutdown.
---

# Rust Axum Service

Use idiomatic Rust and keep the service small enough that its behavior remains
obvious.

## Provenance

- The implementation baseline is `axum_tools_random_api` branch
  `demo-multi-service-observability` at commit `3ea72f3`, which is present in
  this repository's Git history.
- Preserve its proven runtime and observability behavior while improving unsafe
  runtime error handling and extracting recommendation-specific modules where
  that makes behavior clearer.
- Do not reintroduce the historical co-located Python MCP package; that code is
  owned by `movie-recommendation-mcp`.

## Rust Design

- Model bounded states and fault modes with enums rather than free-form strings.
- Use typed request/response structs with Serde at the HTTP boundary.
- Use `Result`, `Option`, and `?` for expected absence/failure; map errors to a
  stable safe HTTP contract at the edge.
- Avoid `unwrap()`/`expect()` on runtime input and external operations.
- Borrow when practical, clone only when ownership or async lifetimes justify
  it, and do not optimize allocation without evidence.
- Introduce traits for real swappable boundaries, not for every function.

## Axum Boundary

- Keep extractors, headers, query parsing, status codes, and response mapping in
  handlers or HTTP adapters.
- Keep recommendation decisions in deterministic functions/modules that do not
  depend on Axum.
- Build a router function that tests can call in-process.
- Bound query limits and user-controlled strings before work or allocation.
- Keep health and readiness cheap and deterministic.

## Tokio And Lifecycle

- Do not block the Tokio runtime with synchronous network/filesystem work.
- Use timeouts around external I/O when introduced.
- Prefer structured task ownership and explicit cancellation/shutdown.
- Use bounded channels if background work needs backpressure.
- Flush telemetry during shutdown, but do not let exporter failure prevent
  process termination indefinitely.

## Errors And Security

- Separate safe client errors from internal diagnostic context.
- Do not log secrets, full provider payloads, or unbounded request content.
- Validate configured endpoints and propagated headers.
- Keep controlled demo faults allowlisted and visibly separate from normal
  recommendation behavior.

## Observability

- Extract W3C trace context at the inbound boundary and preserve correlation
  fields in spans/logs.
- Use low-cardinality metric attributes such as route, outcome, status class,
  and allowlisted fault mode.
- Keep request/trace/user/movie IDs out of metric labels.
- Record enough duration/outcome evidence to explain slow and failed demo paths.

## Workflow

1. Inspect the current route and response contract.
2. Identify ownership, compatibility, and failure semantics.
3. Change the smallest handler/domain/telemetry boundary.
4. Add focused tests for normal, invalid, slow, and failure behavior.
5. Run format, Clippy with warnings denied, and the full test suite.
