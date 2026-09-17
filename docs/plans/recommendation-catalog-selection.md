# Implementation Plan: Recommendation catalog selection

## 1. Summary
Extract ranking into the domain and introduce artifact-selected catalog snapshots.
Remove obsolete request and environment fault controls.

## 2. Goals
Thin HTTP handlers, deterministic ranking, safe provider errors, isolated build
selection, and unchanged health, readiness, authentication and movie listing.

## 3. Non-goals
Other repositories, deployment, operator control APIs, retries, interview material.

## 4. Current State
`services/movie/dummy.rs` mixes catalog data, ranking and orchestration.
`http/mod.rs` applies header-selected delays/errors. `config.rs` enables those
controls. The service already has an asynchronous port and safe HTTP 500 mapping.

## 5. Requirements and Assumptions
This implementation uses artifact selection instead of #15's older runtime
policy proposal. Use a default-off Cargo feature for the separate catalog artifact; select it at
build time, never via request metadata. Each recommendation attempt samples one
snapshot; workflow retries are outside this service. No new dependencies.

## 6. Proposed Design
Domain owns rating calibration, finite-score validation and ranking. The catalog
adapter owns records and random snapshot selection. The existing movie service
coordinates them and records a ranking span. HTTP maps failures to its existing
safe error envelope. Sampling accepts a deterministic substitute in tests.

## 7. Alternatives Considered
Request-level random errors are shallow and disconnect failures from data.
An authenticated runtime policy adds a control plane outside the requested scope.
Artifact-selected catalog data provides an immutable rollout and rollback unit.

## 8. API / Interface Changes
Response shapes, limits and correlation stay stable. `X-Demo-Fault` and legacy
fault environment variables no longer alter outcomes. Remove injected-fault
metrics/events; preserve ordinary request status/duration telemetry.

## 9. Data Model / Persistence Changes
In-memory rating calibration snapshots; no persistence or migration.

## 10. Security, Privacy, and Abuse Considerations
No caller-selected snapshot or random seed. Return generic failures, log only
bounded diagnostic classes, and keep IDs out of metric attributes.

## 11. Performance, Scalability, and Reliability Considerations
Sample once per attempt, no shared mutable RNG or request counters, no retries,
network calls or sleeps. Validate computed scores before sorting/serialization.

## 12. Implementation Steps
1. Extract seed catalog and pure domain ranking; retain success regression tests.
2. Add snapshot data and uniform sampling using existing UUID entropy; exercise
   snapshots directly and deterministically through the service boundary.
3. Remove header/environment controls and test both HTTP outcomes and unaffected
   endpoints with fixed snapshot sources.
4. Add an optional Docker build feature argument and document artifact selection.
5. Update canonical `.ai` boundaries and sync generated guidance; run review/checks.

## 13. Testing Strategy
Domain ordering/preferences, calibration failure, every snapshot, sampling
rejection boundaries, HTTP 200/500, metadata noninterference, limits, health and
readiness. Run format, all-target check, all-feature Clippy and tests with and
without the feature. No statistical acceptance tests.

## 14. Rollout / Migration Plan
Default artifact retains baseline calibration. Build the separate artifact with
`catalog-snapshots`; platform selects its immutable digest. Roll back to the prior
image/default build. Legacy fault settings are ignored. No deployment in this task.

## 15. Risks and Mitigations
| Risk | Impact | Mitigation |
| --- | --- | --- |
| Workflow retries mask dependency failures | Different user-visible rate | Report per-attempt behavior only |
| Wrong artifact selected | Unexpected availability | Explicit build feature and immutable digests |
| Internal data exposed | Disclosure | Safe error envelope and bounded diagnostic class |

## 16. Done Criteria
Both paths proven without randomness in assertions, successful API contracts
preserved, no runtime fault control, guidance synchronized, checks passing.

## 17. Review Checklist
Verify dependency direction, metadata noninterference, error telemetry, default
artifact behavior, deterministic tests and rollback documentation.

## 18. Handoff Prompt for Implementation Agent
Implement this plan within the existing Rust service. Preserve lifecycle and
trace/correlation behavior. Run cargo fmt, check, Clippy and both feature test
configurations. Keep deployment and interview explanations outside this repo.

## Verification

- Default and `catalog-snapshots` builds: 52 tests passed in each configuration.
- Formatting, all-target check and all-feature Clippy with warnings denied passed.
- Automation contracts: 10 tests passed.
- Feature-enabled optimized binary: successful and safe failed responses observed;
  health, readiness, movie listing and SIGTERM shutdown verified locally.
- Read-only architecture review: no material findings.
- Default Docker artifact built successfully; container smoke verified health,
  readiness, successful recommendations, retired controls and non-root runtime.
