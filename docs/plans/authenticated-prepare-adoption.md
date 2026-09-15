# Implementation Plan: Authenticated Prepare Adoption

## 1. Summary

[Issue #11](https://github.com/movie-reservation-platform-lab/movie-recommendation-service/issues/11)
adopts the authenticated prepare contract from
[movie-platform-actions #14](https://github.com/movie-reservation-platform-lab/movie-platform-actions/issues/14)
and merged [PR #18](https://github.com/movie-reservation-platform-lab/movie-platform-actions/pull/18)
at reviewed SHA `036531133bcefd454b5afc0eb55f8ba0328901ea`. The approach follows the
validated [recommendation-MCP canary PR #13](https://github.com/movie-reservation-platform-lab/movie-recommendation-mcp/pull/13).

## 2. Goals

- Coordinate the prepare, evidence, and PR/local security-tool pins.
- Supply the required prepare token and protect the caller contract offline.
- Retain the existing publication and PR security boundaries.

## 3. Non-goals

No application, Docker/dependency, vulnerability-exemption, evidence-schema,
unrelated-action, AWS, admission, deployment, or GitHub-settings changes.

## 4. Current State

Upstream `main` at `4301630` pins both publisher actions and the PR tooling
checkout to `bb40579c285df0b581c48b10f9b34574d5c78639`. Publishing already has
`contents: read` plus its required write permissions, uses v1alpha3 and the
`recommendation-service` identity, and is restricted to canonical main pushes.
The noncanonical production-image scan has only `contents: read`, disables
checkout credential persistence, and never logs in or publishes.

## 5. Requirements and Assumptions

### Confirmed Requirements

- Use reviewed actions SHA `036531133bcefd454b5afc0eb55f8ba0328901ea`.
- Pass `github-token: ${{ github.token }}` to prepare.
- Preserve all existing identities, permissions, event guards, and v1alpha3.
- Revert the coordinated pins and prepare input together for rollback.

### Assumptions

The successful recommendation-MCP canary demonstrates publication and admission
for that caller only. Hosted PR CI cannot execute canonical prepare/publication.

### Open Questions

Private-repository compatibility and live publication remain post-merge checks;
they do not require broader caller changes in this slice.

## 6. Proposed Design

Change the three current tooling pins atomically, add the step-scoped prepare
token input, and strengthen automation tests around exact pins, token wiring,
permissions, and event boundaries. Update only current caller documentation;
retain historical validation revisions as historical evidence.

## 7. Alternatives Considered

### Alternative A: Update Prepare Only

- Pros: Smaller text diff.
- Cons: Leaves the related evidence failure-path and scanner-cleanup hardening
  split across versions.
- Decision: Rejected because the reviewed rollout requires coordinated pins.

### Alternative B: Coordinated Adoption

- Pros: Matches the validated canary and gives publisher and PR tooling one
  reviewed implementation.
- Cons: Requires all three pin references to move together.
- Decision: Selected.

## 8. API / Interface Changes

The prepare composite receives its newly required token input. No service HTTP,
runtime, artifact identity, or evidence-schema interface changes.

## 9. Data Model / Persistence Changes

None.

## 10. Security, Privacy, and Abuse Considerations

Use the existing GitHub job token with unchanged authority. Preserve
`contents: read` and publishing permissions, canonical main-push-only
publication, disabled checkout credentials, and no `pull_request_target`.
Actions PR #18 also adds bounded legacy report handling, sanitized failures,
and scanner-cleanup reporting; caller pin coordination adopts those fixes.

## 11. Performance, Scalability, and Reliability Considerations

Prepare makes one bounded authenticated canonical-main lookup with no anonymous
fallback. Central-policy access and scanner acquisition remain separate live
dependencies. No runtime performance or concurrency behavior changes.

## 12. Implementation Steps

1. Update three reviewed tooling pins and pass the prepare token in
   `.github/workflows/ci.yml`.
2. Extend `automation/tests/release-contract.test.mjs` for exact pin, token,
   permissions, identity, and publication/PR boundary assertions.
3. Update current pin and rollback guidance in `README.md` and the v1alpha3 plan.
4. Run repository-prescribed checks, request read-only security review, commit,
   push, open one PR, and inspect ordinary PR CI.

## 13. Testing Strategy

Run formatting, all-target check, Clippy with warnings denied, all Rust tests,
locked release build, Node automation tests, and diff checks. Contract tests
cover this YAML-only integration; PR CI cannot cover canonical publication.

## 14. Rollout / Migration Plan

Merge only after review and green attributable CI. Identify the next new
successful canonical `main` publication run after merge and admit that exact run
through the environment-owned process. Do not reuse old run IDs. Roll back by
reverting the three pins, new prepare input, and matching current docs/tests
together.

## 15. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---:|---:|---|
| Missing token or pin drift | Publication fails closed | Low | Exact step-scoped contract tests |
| PR gains publication authority | Supply-chain exposure | Low | Exact event and permission assertions |
| Canary success is overstated | Invalid rollout confidence | Medium | Separate PR CI from canonical acceptance |

## 16. Done Criteria

- One bounded issue, branch, commit, and PR.
- Repository-prescribed offline checks pass.
- Current docs describe coordinated rollback and post-merge acceptance.
- No runtime, Docker, dependency, schema, exemption, or unrelated pin changes.

## 17. Review Checklist

- [x] Requirements, non-goals, current conventions, and alternatives inspected.
- [x] Security, reliability, validation, rollout, and rollback specified.
- [x] Final diff and local checks reviewed before handoff.

Offline result (2026-09-15): formatting, all-target check, Clippy with warnings
denied, 56 Rust tests, locked release build, ten workflow contract cases, and
diff checks passed. A bounded read-only review using the repository's
`security-practices` criteria found no material issue: the existing permissions,
canonical guard, credential-disabled PR checkouts, component identity, and
v1alpha3 selection remain unchanged. Independent orchestration review and hosted
PR CI remain separate checks. Canonical prepare/publication is not exercised on
PRs and remains post-merge acceptance.

## 18. Handoff Prompt for Implementation Agent

Implement steps 1-4 with the exact reviewed SHA. Preserve the stated boundaries,
run all checks, and report live canonical acceptance separately. Stop if the
caller requires any application, Docker, dependency, schema, or permission
expansion.
