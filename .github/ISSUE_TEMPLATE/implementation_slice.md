---
name: Implementation slice
about: Track a reviewable recommendation-service change
title: ''
labels: ''
assignees: ''
---

Parent: #

## Goal

What should change, and what outcome should be true when this is done?

## Context

Why is this needed now? Link the plan under `docs/plans/`, the parent epic,
previous PR, or issue that gives the reviewer enough background.

## Scope

- TODO

## Non-goals

- TODO

## Ownership / Learning Mode

Is this normal implementation, hybrid teaching, review-only, or another working
mode? If hybrid teaching applies, name the engineer-owned slice.

## Service Impact

- [ ] No runtime behavior change
- [ ] HTTP contract change to `/health`, `/ready`, `/movies`, or
      `/recommendations` (additive / breaking with coordinated MCP migration)
- [ ] Recommendation ranking or catalog calibration behavior
- [ ] Demo authentication or audit event semantics
- [ ] Observability signals (traces, metrics, structured logs, metric labels)
- [ ] Configuration, environment variables, or Cargo features
- [ ] Concurrency, timeouts, or shutdown behavior
- [ ] Container image, health check, or runtime packaging
- [ ] Dependencies (`Cargo.toml` / `Cargo.lock`)
- [ ] CI or repository automation only

Notes:

## Repository Boundary

- [ ] Service-only; MCP tool contracts stay in `movie-recommendation-mcp`, and
      AWS resources, deployment, and promotion stay in `movie-platform-infra` /
      `movie-platform-environments`
- [ ] Needs a coordinated change in another repository (link it below)

## Acceptance Criteria

- [ ] TODO

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --all-targets --locked`
- [ ] Container smoke (`automation/container-smoke.sh`) if the image changes
- [ ] Other:

## Delivery

Branch: `ai/<issue>-<slug>`

Commit prefix: `[ai][#<issue>]`

## Links

- TODO
