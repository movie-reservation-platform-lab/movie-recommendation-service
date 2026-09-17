---
name: clean-architecture
description: Use when designing, reviewing, or refactoring recommendation-domain, use-case, HTTP, and infrastructure boundaries in this Rust/Axum service. Also use when explicitly requested; skip mechanical edits with no architectural choice.
---

# Recommendation Service Clean Architecture

Adapted from `movie-reservation-service/.ai/skills/clean-architecture/SKILL.md`
for this small Rust/Axum service.

Keep dependencies pointing from HTTP and infrastructure code toward plain
application and domain behavior. Enforce responsibilities before directories;
do not impose a new crate or four-layer layout on a small service.

## Responsibility Ownership

- Recommendation domain: typed movie values, filtering, ranking, bounds, and
  deterministic selection rules. Keep these testable as ordinary Rust functions
  without Axum, Tokio timers, environment access, or telemetry SDKs.
- Application operations: coordinate a recommendation use case and any external
  capabilities it actually needs. Own narrow ports here when substitution or
  separation of I/O from policy provides a concrete benefit.
- HTTP boundary: routes, Axum extractors, query parsing, request/response DTOs,
  headers, status codes, and mapping typed results and errors to responses.
- Infrastructure: catalog access, external clients, configuration loading,
  telemetry exporters, and other concrete I/O mechanics.
- Composition root: startup wires concrete dependencies into router state and
  owns process lifecycle and shutdown. Keep business decisions out of wiring.

Follow existing modules, such as `src/domain/`, `src/services/movie/`, `src/http/`, and
`src/telemetry.rs`, where present. Extract cohesive responsibilities from
`src/main.rs` incrementally; these paths describe ownership, not a mandatory
target tree.

## Rust Boundary Rules

- Keep handlers thin: parse and validate transport input, invoke the operation,
  and map the result. Put semantic recommendation decisions inward.
- Prefer plain functions and concrete structs first. Introduce a small trait,
  function parameter, or enum only for a real boundary or useful substitution.
  Avoid universal repository traits and service base abstractions.
- Let the consumer own the port; an outer adapter implements it. Do not expose
  a full SDK client, Axum state, or database connection through a domain API.
- Choose generics or trait objects from actual ownership and dispatch needs.
  Keep `Arc`, `Send`, `Sync`, and async bounds where sharing and execution
  require them; do not spread them through pure domain values by habit.
- Use typed inputs and errors where they clarify invariants. Keep HTTP status
  codes and `IntoResponse` mapping at the HTTP boundary. Preserve infrastructure
  failure distinctions while returning safe public errors.
- Separate transport DTOs from domain types when their contracts differ. Do
  not duplicate every struct merely to demonstrate layers; serialization
  derives alone do not justify a mapping layer.
- Keep I/O asynchronous and bounded at the adapter/application boundary. Keep
  pure selection logic synchronous; do not introduce async traits speculatively.

## Preserve Service Contracts

- Preserve `/health`, `/ready`, `/movies`, and `/recommendations`, including
  defaults, bounds, response shapes, and error behavior during extraction.
- Keep snapshot sampling in the catalog adapter and calculation/ranking rules
  in the domain. Domain failures must propagate through the application service
  and be mapped to safe responses at HTTP. Do not put random decisions, sleeps,
  runtime fault switches, or request-header parsing in recommendation rules.
- Request-selected faults are retired. Alternate catalog data is selected by
  the immutable artifact, never by caller metadata.
- Preserve W3C trace propagation and bounded request/correlation context at
  boundaries without coupling domain objects to OpenTelemetry.
- Keep metric labels bounded and shutdown/telemetry flushing finite.
- Keep MCP contracts, agent orchestration, and deployment ownership outside
  this service.

## Testing And Refactoring

1. Identify the behavior and the responsibility that owns each decision.
2. Protect relevant observable behavior before moving it.
3. Extract one meaningful boundary while preserving the public contract.
4. Test domain rules directly with ordinary values; test application operations
   with small fakes only when ports exist.
5. Test HTTP mapping through an in-process Axum router and adapters at their I/O
   boundary. Use deterministic time for controlled delays where supported.
6. Review dependency direction and whether each abstraction earns its cost.

Use the repository's `rust-axum-service` and `rust-testing` skills for detailed
runtime and testing conventions, and follow its planning requirements for
contract, concurrency, observability, dependency, or artifact changes.

Avoid broad rewrites whose primary result is a directory diagram. A module
with pure functions and a thin handler can already have sound boundaries.
