# Implementation Plan: container evidence admission parity

## 1. Summary

Issue #8: adopt the organization-owned container evidence actions, independently of runtime changes.

## 2. Goals

Publish signed, subject-bound security evidence for recommendation-service, sufficient for independent environment verification.

## 3. Non-goals

AWS changes, deployment, environment selection, runtime changes, static OCI admission, replacing business tests or permanent security policy.

## 4. Current State

.github/workflows/ci.yml owns publish-image, quality/smoke prerequisites and its existing display name. automation/tests/release-contract.test.mjs protects publication behavior. The current image attestation is not a complete downloadable security-evidence package.

## 5. Requirements and Assumptions

Keep canonical push/main publication and existing quality/check identities. Add repository guard, non-cancelling publication, attempt tags, single linux/amd64 provenance:false build, full SHA actions and exact digest evidence. Node 24 is installed in the publisher, with no producer dependency install there. Live scans and environment App/IAM readiness remain operational questions, not assumptions of success.

## 6. Proposed Design

Build stays in publish-image; shared preparation runs before login/build and evidence consumes its digest afterwards. The pinned composite owns verification/scans/policy/emission/attestations/upload. Environments owns admission decisions. No new runtime abstraction is needed; boundaries stay outside application code.

## 7. Alternatives Considered

Copy reservation-service tooling here: initially simple but duplicates security implementation; rejected. Reusable workflow: reduces YAML but changes signer/check boundaries; defer. Adopt pinned composite within existing job: chosen, keeps repository-owned gates and independently reviewable rollback.

## 8. API / Interface Changes

New ci.movie-platform.dev/v1alpha2 four-file candidate package. Artifact: recommendation-service-security-evidence-RUN-attempt-ATTEMPT. Discovery tag gains run/attempt suffix; immutable digest remains identity. Job display name is retained and recorded in evidence. Shared contract documentation is in movie-reservation-platform-lab/movie-platform-actions#2 and this producer pins commit 9b7b5a601367a45356687a0e1bf1d1638d62aca9.

## 9. Data Model / Persistence Changes

None to runtime data. Evidence expires after 14 days. Older successful runs cannot be reconstructed or admitted without their required evidence.

## 10. Security, Privacy, and Abuse Considerations

Guard canonical repository, never publish from PRs/forks/dispatch. Keep write permissions only in the publisher, checkout credentials disabled, explicit immutable pins, no AWS secrets/config. Shared code is supply-chain code requiring review. Missing provenance, mismatched subject or CRITICAL findings fail closed.

## 11. Performance, Scalability, and Reliability Considerations

Bound publication to 40 minutes including two five-minute scans. Preserve PR cancellation but serialize push/main without cancellation across image push and attestation. Attempt-specific tags avoid rerun ambiguity. Actual latency/scan results must be measured after merge.

## 12. Implementation Steps

1. Update .github/workflows/ci.yml guards, concurrency and action pins; preserve quality/build inputs.
2. Add prepare/evidence calls pinned to shared #13; use exact build digest.
3. Update automation/tests/release-contract.test.mjs to assert shared interface, pin consistency, ordering and permission boundaries.
4. Run repository checks; independently review before PR. Do not dispatch live publication.

## 13. Testing Strategy

Run: node --test automation/tests/*.test.mjs; cargo fmt --all -- --check; cargo clippy --all-targets --all-features --locked -- -D warnings; cargo test --all-targets --locked.
Keep contract tests separate from runtime tests. Shared action tests cover behavior; repository tests cover caller wiring. Real OIDC/registry acceptance requires a canonical successful main run after merge.

## 14. Rollout / Migration Plan

Depends on https://github.com/movie-reservation-platform-lab/movie-platform-actions/pull/2 at commit 9b7b5a601367a45356687a0e1bf1d1638d62aca9. Merge the shared action first, then this PR. Add matching environment verification separately. Record fresh run/attempt and immutable digest; do not infer deployment. Rollback by reverting this workflow/pin change without changing runtime code.

## 15. Risks and Mitigations

CRITICAL vulnerabilities may block first run: inspect rejected diagnostics, never weaken gate. Static/image confusion: explicit job/profile. Shared drift: full SHA pin. Registry copy alone is not equivalent to admission; consumer rollout must close that gap.

## 16. Done Criteria

Local workflow and regression checks green, reviewed public diff, shared pin verified, no sensitive files. Live success is a later acceptance step, not claimed by this PR.

## 17. Review Checklist

- [x] Scope, ownership, alternatives, compatibility and rollback explicit.
- [x] Repository-specific build and test targets inspected.
- [x] Final tests and independent review complete (local checks passed; no live publication).
- [ ] Live canonical acceptance (operator rollout).

## 18. Handoff Prompt for Implementation Agent

Implement this plan in .github/workflows/ci.yml and automation/tests/release-contract.test.mjs. Preserve runtime behavior and stable quality checks. Use the shared reviewed commit, update docs and run the commands above. Do not publish images, change AWS configuration or admit/deploy candidates as part of PR preparation.
