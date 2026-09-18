# Implementation Plan: Recommendation service observability signals

Tracking: [issue #2](https://github.com/movie-reservation-platform-lab/movie-recommendation-service/issues/2).

## 1. Summary

Verify the existing HTTP metrics and traces with in-memory exporter evidence,
remove the retired fault metric dimension, and make request events carry the
resource and active-span identity required for log correlation.

## 2. Goals

- Prove exact metric names, units, temporality, resources and bounded attributes.
- Preserve successful, rejected and failed request semantics.
- Correlate JSON request events with their active trace/span and release identity.

## 3. Non-goals

- Collector, dashboard, deployment or MCP changes.
- New business metrics, retries, runtime fault controls or public API changes.

## 4. Current State

`src/telemetry.rs` emits request counters, duration histograms and returned-item
counters. `src/http/mod.rs` records every fixed route and existing tests prove
safe errors and failure-span parenting. Metrics still attach the retired
`demo.fault=none` dimension, application events omit the active span ID and
resource identity, and no test captures the SDK-exported metric payload.

## 5. Requirements and Assumptions

### Confirmed Requirements

- Follow the advisory infra #57 signal contract and report observed native output.
- Keep labels bounded and keep telemetry failure off the request path.
- Preserve health, readiness, movie and recommendation behavior.

### Assumptions

- Health/readiness series remain emitted and are excluded from user-path queries
  using the fixed route label.
- SDK cumulative temporality observed through the test exporter is the local
  evidence; AMP translation remains infrastructure-owned.

### Open Questions

- None blocking producer implementation. Infra must confirm translated AMP names.

## 6. Proposed Design

Use the existing telemetry adapter. Replace the obsolete fault attribute with
bounded method, status-class and outcome attributes while retaining the native
route and numeric status attributes. Build JSON event envelopes with release,
environment, severity, timestamp and active span identity. Add an in-memory SDK
exporter test that exercises success, 4xx and 5xx series and checks the exact
resource, counter and histogram data.

## 7. Alternatives Considered

### Keep only unit tests of label builders

- Pros: small.
- Cons: does not prove SDK output, units, aggregation or resource identity.
- Decision: rejected.

### Capture a live collector payload

- Pros: closest to deployment.
- Cons: slow, environment-dependent and belongs to infra acceptance.
- Decision: defer; use credential-free in-memory exporter evidence here.

## 8. API / Interface Changes

No HTTP changes. Metric attributes and JSON log fields become the documented
producer contract.

## 9. Data Model / Persistence Changes

None.

## 10. Security, Privacy, and Abuse Considerations

Only fixed route/method/outcome values become metric dimensions. Request and
trace IDs remain log/span fields. No prompts, request bodies or secrets are logged.

## 11. Performance, Scalability, and Reliability Considerations

Metrics retain constant-cardinality dimensions. Timestamp and identity fields
add bounded JSON data only. Export remains asynchronous and fail-open.

## 12. Implementation Steps

1. Refine bounded metric attributes in `src/telemetry.rs`.
2. Add resource-aware correlated request-event envelopes in `src/telemetry.rs`
   and active span extraction in `src/http/mod.rs`.
3. Add in-memory metric exporter and trace/log contract tests.
4. Document the observed payload and query exclusions in `docs/observability.md`.

## 13. Testing Strategy

Run focused telemetry and HTTP tests, then format, check, Clippy, all tests and
both canonical catalog build configurations.

## 14. Rollout / Migration Plan

Publish an immutable image independently. Infra reconciles collector filters
before coordinated selection. Roll back to the previous digest and queries.

## 15. Risks and Mitigations

| Risk | Mitigation |
| --- | --- |
| Dashboard expects a retired label | Preserve instrument names and document exact new attributes before infra #58. |
| Missing data appears healthy | Document zero/idle/stale semantics; do not manufacture observations. |
| Log IDs are synthetic | Read trace/span IDs only from an active valid span. |

## 16. Done Criteria

- Exporter-backed evidence covers 200, 400 and 500 request series and latency.
- Failure traces remain connected and correlated events include active IDs.
- Exact native payload semantics are documented and repository checks pass.

## 17. Review Checklist

- [x] Requirements and non-goals are explicit.
- [x] Existing instrumentation and tests were inspected.
- [x] Security, cardinality, testing and rollback are covered.
- [x] Steps name concrete files and preserve public behavior.

## 18. Handoff Prompt for Implementation Agent

```text
Implement docs/plans/issue-2-observability-signals.md in this repository only.
Preserve public HTTP behavior and existing native instrument names. Use bounded
attributes and credential-free exporter evidence. Run Rust formatting, check,
Clippy and tests. Do not deploy or change infrastructure.
```
