---
name: rust-testing
description: Use when writing or reviewing Rust tests for this recommendation service, including Axum handlers, catalog and ranking failures, test doubles and fixtures, Cargo test-support features, paused Tokio time, telemetry contracts, and container health.
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

## Test doubles and fixtures

- Run real domain logic and application services. For an existing port with
  shared ownership, inject a hand-written double through `Arc<dyn Port>`.
- Use a **fake** for meaningful behaviour or state, such as a repository that
  stores records. Use a **stub** for a fixed result or forced error.
- Use a **spy** to record an outbound action, or a **mock** to check an expected
  interaction, when that interaction is the contract. Consider a mocking crate
  when repeated contract expectations would be harder to maintain by hand.
- Give each test fresh state. Keep fixture builders and router setup local until
  multiple modules use them. Share setup while keeping inputs and assertions
  visible. Name production in-memory adapters for their role, such as
  `InMemoryCatalog`, and reserve `FakeCatalog` for a test double.

## Placement and ownership

- Put tests that need private access in a child `#[cfg(test)] mod tests`.
  Share helpers across unit modules through a crate-root `#[cfg(test)]`
  support module. Keep test-only dependencies and features in dev-dependencies.
- Put public library API or process tests in `tests/` and share their helpers
  via `tests/support/mod.rs`. Integration crates cannot import private items
  or binary internals. Keep binary-only router tests in the binary crate unless
  there is a reason to expose a library API.
- Publish a producer-owned fake when multiple consumers need the same stable
  contract. Use a default-off feature and run common contract cases against the
  fake and the real implementation. At an HTTP boundary, publish versioned
  fixtures or schemas that the producer checks against its API. Consumers can
  serve matching responses through a fake endpoint or in-process transport.
- See [Test support](references/test-support.md) for a worked example, feature
  gates, release verification, and the lint matrix.

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
- Capture emitted telemetry with an in-memory OpenTelemetry exporter or a scoped
  tracing subscriber. Flush buffered exports before asserting the captured data.
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
