# Implementation Plan: v1alpha3 evidence and production-image PR scanning

## 1. Summary
Resume issue #8 / PR #10 and include the engineer's uncommitted hybrid-teaching guidance. The initial rollout adopted reviewed shared-actions `bb40579c285df0b581c48b10f9b34574d5c78639`, following merged recommendation-MCP PR #11 (`7b56272`). The current pin is maintained by the [authenticated prepare adoption](authenticated-prepare-adoption.md); historical validation results below retain the revision actually tested.

## 2. Goals
Use v1alpha3 exact-digest publication evidence and read-only production-image checks before merge. Build, smoke-test and scan this Rust service; fix its measured vulnerabilities and retain complete diagnostics on rejection.

## 3. Non-goals
MCP runtime changes, service API changes, image publication during PR preparation, merge, AWS changes, environment admission, policy exemptions or weakening the central gate.

## 4. Current State
The issue branch is in the existing `movie-recommendation-service-evidence` worktree. Its workflow pins v1alpha2 actions at `9b7b5a6`, with canonical main/push/repository guards and write permissions confined to `publish-image`. Dockerfile builds Rust on Bookworm and runs on Bookworm slim with curl, certificates and tini. The smoke script covers health/readiness, normal/slow/error recommendations and non-root execution. The original main worktree has uncommitted hybrid-teaching metadata, skill files and generated AGENTS.md.

## 5. Requirements and Assumptions
The user explicitly authorizes implementation, commits and updating existing PR #10, including current local changes. Preserve the originals while copying them into the issue worktree. Shared actions PR #2 and MCP PR #11 are merged. Current central approvals are fetched by the reviewed evaluator; do not assume scan results. Findings determine whether Dockerfile or Cargo.lock remediation is needed.

## 6. Proposed Design

Measured baseline: the Bookworm image failed with 4 CRITICALs and 67 HIGHs. The CRITICALs are three perl-base findings (CVE-2026-13221, CVE-2026-42496, CVE-2026-8376) and zlib1g CVE-2023-45853. Bookworm package candidates offer no newer versions. Use Trixie for the runtime and refresh its preinstalled packages before installing runtime dependencies. The initial Trixie scan removed all CRITICALs but still reported available fixes for gzip, glibc, PCRE2 and SQLite; upgrading preinstalled packages remediates those as well as perl-base. Keep the Bookworm Rust builder, curl readiness check, certificates, tini and UID 10001. Smoke-test the binary against the newer runtime libraries. Do not suppress the zlib finding or request an exemption.
Add `container-security-check`, matching MCP's read-only noncanonical-event job, with checkout of the exact reviewed tooling SHA, Node 24, a linux/amd64 `runtime` image build and the shared v1alpha3 local scanner. Upload the complete output directory under a diagnostics-only name with `!cancelled()` so a failed scan still uploads. Keep canonical publication's exact-digest scan in its existing job, opting into v1alpha3 and pinning both composites to the same SHA.

## 7. Alternatives Considered
- Copy MCP Dockerfile: rejected; Python dependency/runtime choices do not apply to Rust or its curl health check.
- Duplicate the evaluator/scanner: rejected; policy drift and extra supply-chain maintenance.
- Reuse reviewed shared tooling and remediate measured Rust-image packages: selected; small caller changes and consistent policy behavior.

## 8. API / Interface Changes
Candidate evidence moves to `ci.movie-platform.dev/v1alpha3`. Existing HTTP contracts and publisher identity remain stable. PR reports are diagnostics, never signed candidate evidence. Local tool exit 0 means pass, 1 policy rejection, 2 incomplete evaluation.

## 9. Data Model / Persistence Changes
No runtime data changes. CI diagnostics retain complete findings for 14 days; local reports live outside Git.

## 10. Security, Privacy, and Abuse Considerations
Only canonical push/main can log in, publish or attest. PR job has `contents: read`, credential persistence disabled, no registry login or write/OIDC permissions. Gate errors propagate. Full all-severity reports include unfixed findings; no producer-local ignores or exemptions. Preserve TLS certificates, non-root UID and bounded shutdown.

## 11. Performance, Scalability, and Reliability Considerations
PR scan gets a bounded job timeout sufficient for Rust release compilation and the shared bounded scanner/evaluator. Canonical main does not duplicate the local scan; exact published digest is scanned by the evidence action. Build cache is optional, not a correctness dependency.

## 12. Implementation Steps
1. Copy local canonical guidance into the existing issue worktree; regenerate AGENTS.md and compare to original.
2. Update `.github/workflows/ci.yml` and `automation/tests/release-contract.test.mjs` for pins, v3, permission/event boundaries and failure-safe upload.
3. Build the current `runtime` image and scan with reviewed shared tooling; remediate only measured package/dependency findings in Dockerfile/Cargo.lock as required. Rebuild and scan the final image, smoke-test its HTTP, health and lifecycle behavior.
4. Run repository Rust and automation checks. Run pinned shared scanner/evidence failure tests to verify complete reports survive rejection and rejected evidence cannot create a candidate. Ask a read-only security review agent for findings with file/line evidence.
5. Update README and validation notes. Commit with `[ai]`, push the existing branch and update PR #10 title/body/dependency notes. Inspect hosted PR checks and downloadable reports.

## 13. Testing Strategy
`node --test automation/tests/*.test.mjs`; `cargo fmt --all -- --check`; `cargo check --all-targets --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-targets --locked`; `cargo build --release --locked`; Docker build checks, production build and `automation/container-smoke.sh`; exact reviewed shared scanner and rejection test suites. Inspect hosted PR publication skip and complete artifact files.

## 14. Rollout / Migration Plan
Update PR #10 only. Shared implementation is already merged; environment-owned v3 admission remains separate (movie-platform-environments #82). Fresh canonical main publication is required after merge; older runs are not retroactively admissible. Roll back by reverting workflow/image changes without changing service interfaces.

## 15. Risks and Mitigations
- Existing base or Rust dependencies contain CRITICALs: inspect full report, update actual affected dependencies, rescan.
- Gate suppresses diagnostics: explicit status condition plus upstream failure-path tests and hosted artifact inspection.
- Local pass mistaken for admission: label all local/PR output diagnostic only; retain image ID, report hash and policy revision.

## 16. Done Criteria
Local changes preserved/included, reviewed pin used consistently, final Rust production image built/smoked/scanned, blocking findings remediated, regression checks and review complete, PR #10 updated with concrete validation and remaining canonical acceptance dependency.

## 17. Review Checklist
- [x] Scope, existing code, alternatives, permission boundaries and rollback inspected.
- [x] Implementation steps and verification targets explicit.
- [x] Final scan, regression suite and independent review complete.
- [ ] PR update and hosted diagnostics verified.

## 18. Handoff Prompt for Implementation Agent
Implement this plan in the existing issue worktree; use the reviewed shared SHA and Rust runtime target, preserve the engineer's changes, measure findings before remediation, retain full diagnostics, run the stated checks and update PR #10 with `[ai]` commits/title. Do not merge, publish an image manually or change environments.


## Validation results (2026-09-15)

- Format, all-target check, Clippy with warnings denied, 56 Rust tests and locked release build passed.
- Nine automation contract tests passed. Docker build checks reported no warnings.
- Reviewed shared runtime/local-tool suites: 56 tests passed, including real CLI rejection with complete retained reports, failed policy acquisition, rejected hosted candidates, and PR-context publication refusal.
- Actual baseline scan returned exit 1 with 4 CRITICALs / 67 HIGHs; its complete 320-finding `vulnerabilities.json`, `vulnerability-policy.json` and `summary.txt` survived. Report SHA-256: `4abc8c3f41eab9997a4d80954f8f1cdaad99cedcd5f5a34491f540e4aae654e2`.
- Final linux/amd64 runtime image: `sha256:12c0289085ea827d546a3f2d086eee03e4b73e669983962d75bd049f7327e4dd` (local Docker image ID, not a published GHCR candidate).
- Final Trivy 0.70.0 report: **0 CRITICAL, 51 HIGH, 81 MEDIUM, 90 LOW, 1 UNKNOWN**; policy passed with zero exemptions at `2026-09-15T07:27:28Z`, revision `bb40579c285df0b581c48b10f9b34574d5c78639`. No remaining finding lists an available fixed version. Complete report SHA-256: `cc760b02c1f42f8c5940de2ccdaad5008c894bdbbe548fbf8a04b31cdf3a5da5`.
- Final smoke covered health/readiness and normal/slow/error recommendations. Docker's own readiness health check passed; UID was 10001; SIGTERM exited 0 in 0.40 seconds.
- Installed fixes: perl-base `5.40.1-6+deb13u1`, zlib1g `1:1.3.dfsg+really1.3.1-1+b1`, gzip `1.13-1+deb13u1`, libc6 `2.41-12+deb13u4`, PCRE2 `10.46-1~deb13u2`, SQLite `3.46.1-7+deb13u2`.
- Read-only security review found no material issue. Hosted upload and fresh canonical publication are separate operational checks; PR diagnostics never authorize admission.
- The original worktree's four uncommitted guidance files match the included copies byte-for-byte and remain untouched in the original worktree.

Local reports are retained outside Git under `/tmp/recommendation-service-issue8-baseline-scan/run-ludu5J/` and `/tmp/recommendation-service-issue8-remediated-scan/run-2VPCiE/`.

Primary remediation references: Debian's [Perl status](https://security-tracker.debian.org/tracker/CVE-2026-42496) and [zlib status](https://security-tracker.debian.org/tracker/CVE-2023-45853). No exception was used for the Bookworm zlib source-package finding.
