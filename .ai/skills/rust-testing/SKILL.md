---
name: rust-testing
description: Use when creating, organizing, refactoring, reviewing, or explaining tests for this Rust recommendation service, including pure unit tests, in-process Axum handler tests, Tokio async and time tests, catalog/ranking failure cases, trace and metric contracts, and health/container behavior.
---

# Rust Testing

Choose the smallest test boundary that proves the behavior.

Treat router and fault-path tests from `axum_tools_random_api@3ea72f3` as the
starting regression suite. Preserve current contracts while replacing retired request-fault tests with
deterministic catalog/error cases.

## Test Layers

- Unit tests: recommendation rules, bounds/defaults, rating calibration, trace
  parsing, and response/error mapping.
- Handler tests: call an in-process Axum `Router` with `tower::ServiceExt` and
  assert status, headers, and JSON without binding a port.
- Async tests: use `#[tokio::test]` only when behavior is genuinely async.
- Smoke tests: start the built process/container separately to prove health,
  readiness, shutdown, and exporter configuration.

## Determinism

- Use table-driven cases for bounds, defaults, and catalog snapshot outcomes.
- Prefer paused Tokio time for controlled delays and timeout behavior.
- Avoid real sleeps and external network calls in ordinary tests.
- Give each test fresh state; serialize only tests that truly share global
  telemetry/process state.

## Observability Coverage

- Test traceparent parsing and rejection of malformed/all-zero IDs.
- Assert stable response correlation fields where public.
- Test metric attribute construction as bounded data rather than relying on a
  live collector.
- Keep exporter integration as a separate smoke boundary.

## Failure Coverage

- Invalid and extreme query inputs.
- Every catalog snapshot, successful and failed ranking, metadata noninterference.
- Canonical-build success and failure paths; no statistical assertions.
- Missing or malformed propagation headers.
- Telemetry disabled or exporter setup failure.
- Graceful shutdown and health/readiness contracts where changed.

Run focused tests during development, then `cargo fmt --all -- --check`, Clippy
with warnings denied, and `cargo test --all-targets`.
