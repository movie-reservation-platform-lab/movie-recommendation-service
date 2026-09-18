# Implementation Plan: Recommendation catalog selection

## 1. Summary
Extract ranking into the domain and introduce catalog calibration snapshots.
Remove obsolete request and environment fault controls.

## 2. Goals
Thin HTTP handlers, deterministic ranking, safe provider errors, and unchanged
health, readiness, authentication and movie listing.

## 3. Non-goals
Other repositories, deployment, operator control APIs, retries, interview material.

## 4. Current State
`services/movie/dummy.rs` mixes catalog data, ranking and orchestration.
`http/mod.rs` applies header-selected delays/errors. `config.rs` enables those
controls. The service already has an asynchronous port and safe HTTP 500 mapping.

## 5. Requirements and Assumptions
The canonical service samples one catalog snapshot per recommendation attempt.
Callers cannot select or bypass snapshots through request metadata. Workflow
retries are outside this service. No new dependencies.

## 6. Proposed Design
Domain owns rating calibration, finite-score validation and ranking. The catalog
adapter owns records and random snapshot selection. The existing movie service
coordinates them and records a ranking span. HTTP maps failures to its existing
safe error envelope. Sampling accepts a deterministic substitute in tests.

## 7. Alternatives Considered
Request-level random errors are shallow and disconnect failures from data.
An authenticated runtime policy adds a control plane outside the requested scope.
Catalog data in the canonical release resembles an ordinary production defect;
the previously published healthy digest remains the rollback unit.

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
4. Update the canonical container smoke contract for both valid recommendation
   outcomes without statistical assertions.
5. Update canonical `.ai` boundaries and sync generated guidance; run review/checks.

## 13. Testing Strategy
Domain ordering/preferences, calibration failure, every snapshot, sampling
rejection boundaries, HTTP 200/500, metadata noninterference, limits, health and
readiness. Run format, all-target check, Clippy and the full test suite. No
statistical acceptance tests.

## 14. Rollout / Migration Plan
The existing canonical main-push workflow publishes the release after its normal
quality, smoke, vulnerability, provenance, and evidence gates. Roll back by
selecting the previously published healthy digest. Legacy fault settings remain
ignored. No deployment in this task.

## 15. Risks and Mitigations
| Risk | Impact | Mitigation |
| --- | --- | --- |
| Workflow retries mask dependency failures | Different user-visible rate | Report per-attempt behavior only |
| Wrong digest selected | Unexpected availability | Use immutable publication and rollback digests |
| Internal data exposed | Disclosure | Safe error envelope and bounded diagnostic class |

## 16. Done Criteria
Both paths proven without randomness in assertions, successful API contracts
preserved, no runtime fault control, guidance synchronized, checks passing.

## 17. Review Checklist
Verify dependency direction, metadata noninterference, error telemetry,
deterministic tests and rollback documentation.

## 18. Handoff Prompt for Implementation Agent
Implement this plan within the existing Rust service. Preserve lifecycle and
trace/correlation behavior. Run cargo fmt, check, Clippy and the full test suite.
Keep deployment and interview explanations outside this repo.

## Verification

- Canonical build: 52 tests passed.
- Formatting, all-target check and all-feature Clippy with warnings denied passed.
- Automation contracts: 13 tests passed.
- Optimized binary: successful and safe failed responses observed;
  health, readiness, movie listing and SIGTERM shutdown verified locally.
- Read-only architecture review: no material findings.
- Canonical Docker artifact built successfully; container smoke verified health,
  readiness, recommendation response contracts, retired controls and non-root runtime.
- Existing canonical publication job, tag, scan, provenance and evidence identities
  remain unchanged; no shared-action or environments contract changes are required.
