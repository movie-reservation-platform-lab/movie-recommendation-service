# Implementation Plan: Movie Recommendation API

## 1. Summary

Build the Axum service into the demo recommendation dependency for the multi-service observability flow. The recommended approach is to keep the service fake and deterministic, add a small in-memory movie catalog copied from the sibling `golden-path-ecs-template` seed data, expose `/health`, `/movies`, and `/recommendations`, and implement the poison-pill fault contract only on `/recommendations`.

This plan covers the full target from the upstream service brief. The first implementation slice is the Rust Axum API because it is the service boundary requested for this repo and the prerequisite for any MCP wrapper.

## 2. Goals

- Run the Rust Axum API on port `8082` by default.
- Preserve a simple `/movies` endpoint for catalog inspection.
- Add `GET /health` for health checks.
- Add `GET /recommendations?limit=5` with deterministic recommendation data.
- Seed the fake service with most or all movies from `golden-path-ecs-template`.
- Include `movie_reservation_movie_id` in recommendation output so the agent can map recommendation context to the reservation service catalog.
- Implement `X-Demo-Fault` and `DEMO_FAULT_MODE` handling for `none`, `slow-recommendation`, and `recommendation-error`.
- Emit structured stdout logs with the fields needed for correlation during the demo.
- Emit Rust OpenTelemetry traces and custom metrics for inbound requests, recommendation handling, and poison-pill branches.
- Add the FastMCP wrapper in this repo on the same branch.
- Keep the implementation small and dependency-light.

## 3. Non-goals

- No external movie APIs, TMDB credentials, or network-backed catalog lookup.
- No persistence layer, migrations, cache, or background jobs.
- No recommendation machine learning or personalized scoring.
- No auth/authz in this internal demo service.
- No random fault injection outside the explicit poison-pill contract.
- No live reservation-service lookup from the recommendation API.
- No full personalized ranking model; use a deterministic heuristic only.

## 4. Current State

- The repo is on branch `demo-multi-service-observability`.
- `src/main.rs` exposes only `GET /movies` and binds to `0.0.0.0:8080`.
- `src/di/movie_service.rs` selects `FakeMovieService` only when `USE_DUMMY=true`; otherwise it panics.
- `src/services/movie/dummy.rs` contains two hardcoded movies: `The Matrix` and `Interstellar`.
- `src/domain/movie.rs` has a small serialized `Movie` shape with numeric `id`, `title`, `overview`, `release_date`, and `vote_average`.
- There are no tests in the repo yet.
- `Cargo.toml` already contains `axum`, `tokio`, `serde`, `serde_json`, `anyhow`, `async-trait`, `reqwest`, and `rand`.
- The upstream service brief is `/home/patex1987/development/golden-path-ecs-template/docs/plans/demo-multi-service-observability/axum-tools-random-api.md`.
- The sibling demo seed source is `/home/patex1987/development/golden-path-ecs-template/movie-reservation-service/src/infrastructure/fixtures/movie-reservations/movie-reservation-demo-data.ts`.
- The sibling seed movies are:
  - `The Shawshank Redemption`
  - `The Matrix`
  - `Star Wars: Episode V - The Empire Strikes Back`
  - `Star Wars: Episode IV - A New Hope`
  - `The Dark Knight`
  - `Arlington Road`
  - `The Type-Safe Matinee`
  - `Fargate at Midnight`
  - `The Last Deployment`
  - `Casablanca`

## 5. Requirements and Assumptions

### Confirmed Requirements

- Use the project planner instructions from `/home/patex1987/development/golden-path-ecs-template/.ai/skills/principal-engineer-planner`.
- Create this plan in the current repo under `docs/plans`.
- Implement an Axum movie recommendation API.
- Use seed data containing most of the films from the `golden-path-ecs-template` project.
- Follow the upstream demo contract where practical:
  - `GET /health`
  - `GET /movies`
  - `GET /recommendations?limit=5`
  - `X-Demo-Fault` preferred over `DEMO_FAULT_MODE`
  - `none`, `slow-recommendation`, `recommendation-error`

### Assumptions

- For the first implementation slice, "create a movie recommendation api in axum" means the Rust API, not the Python FastMCP wrapper.
- The recommendation service is an internal demo dependency, so no authentication is required.
- Recommendation order should be deterministic and stable between runs.
- Unknown fault values should be treated as `none` rather than returning a client error, to keep the demo resilient.
- `PORT` should override the default port so Docker Compose can set `8082` explicitly.
- `/movies` can evolve its JSON shape because there is no documented external consumer yet.

### Resolved Decisions

- Implement the FastMCP wrapper in this repo on the same branch.
- Add Rust OpenTelemetry trace spans and custom metrics, not only structured logs.
- Use deterministic heuristic recommendation ranking; do not add a live reservation API dependency for seat availability.

## 6. Proposed Design

The Rust service remains a small in-memory Axum app.

`FakeMovieService` owns the seeded catalog and exposes deterministic methods for:

- listing movies;
- mapping movies to recommendation payloads;
- applying heuristic ranking and the requested result limit.

`main.rs` owns HTTP concerns:

- app state injection through `Arc<dyn AsyncMovieService>`;
- route registration;
- `PORT` parsing;
- header/env fault selection;
- response status mapping;
- structured request/result logs.

The poison pill is implemented only inside the `/recommendations` handler:

- `none`: return recommendations normally;
- `slow-recommendation`: sleep for two seconds, then return success;
- `recommendation-error`: return HTTP 503 and a compact JSON error.

This keeps the failure source easy to explain and easy to find in traces/logs. The service does not call other APIs in this slice, so there are no dependency spans beyond the inbound handler/fault branches.

Rust telemetry is initialized during startup:

- OTLP HTTP/protobuf trace export when `OTEL_EXPORTER_OTLP_ENDPOINT` is configured;
- OTLP metric export with periodic reader when the exporter can be built;
- stdout tracing fallback remains active through `tracing-subscriber`;
- custom metrics record request count, request latency, recommendation count, and fault count.

The FastMCP wrapper is a thin Python service under `axum-tools-mcp/`. It owns only MCP tool surfaces, header propagation, structured logs, and downstream HTTP calls to the Rust API. It must not invent separate faults.

## 7. Alternatives Considered

### Alternative A: Static handler data only

- Pros: Fastest and smallest implementation.
- Cons: Bypasses the existing service abstraction and makes `/movies` and `/recommendations` drift easily.
- Decision: Rejected. The existing fake service is enough structure and keeps catalog ownership in one place.

### Alternative B: Extend the existing fake service

- Pros: Fits current repo shape, keeps data deterministic, makes tests straightforward, avoids external dependencies.
- Cons: The service trait grows slightly before there are multiple implementations.
- Decision: Recommended for the first slice.

### Alternative C: Add reservation-service or TMDB calls

- Pros: Could produce recommendations from live availability or richer metadata.
- Cons: Adds credentials, network failures, retry policy, and another fault source, which conflicts with the demo requirement.
- Decision: Rejected for this demo.

## 8. API / Interface Changes

### `GET /health`

Response:

```json
{
  "status": "ok",
  "service_name": "axum-tools-random-api"
}
```

### `GET /movies?limit=10`

Response is a JSON array of seeded movies. Each movie includes:

- `id`
- `title`
- `overview`
- `release_date`
- `vote_average`
- `movie_reservation_movie_id`
- `rating`
- `duration_minutes`

### `GET /recommendations?limit=5&preference=optional`

Response:

```json
{
  "recommendations": [
    {
      "id": "recommendation-the-matrix",
      "title": "The Matrix",
      "reason": "Matches the demo preference and has available screenings",
      "confidence": 0.95,
      "movie_reservation_movie_id": "44444444-4444-4444-8444-444444444435"
    }
  ]
}
```

### Fault Behavior

- Header: `X-Demo-Fault`
- Env fallback: `DEMO_FAULT_MODE`
- Supported values: `none`, `slow-recommendation`, `recommendation-error`

`recommendation-error` returns:

```json
{
  "error": {
    "code": "recommendation_unavailable",
    "message": "Recommendation service unavailable for demo fault",
    "fault": "recommendation-error"
  }
}
```

## 9. Data Model / Persistence Changes

No persistence changes.

The in-memory `Movie` model gains optional reservation-service mapping fields:

- `movie_reservation_movie_id`
- `rating`
- `duration_minutes`

Recommendation payloads are derived from the in-memory movies and do not persist state.

## 10. Security, Privacy, and Abuse Considerations

- Do not log request bodies or arbitrary headers.
- Log only bounded demo fault values, route, ids, status, duration, and service name.
- Clamp `limit` to a bounded maximum to avoid accidental large responses if the catalog grows.
- Treat unknown `X-Demo-Fault` values as `none` so callers cannot create unexpected failure modes.
- No secrets are required.
- This service is intended for local/internal demo use; external exposure would require auth, rate limits, and stricter CORS/proxy policy.

## 11. Performance, Scalability, and Reliability Considerations

- The data set is small and in memory, so latency should be stable.
- The slow fault intentionally sleeps for two seconds and should be used only for demo traffic.
- No shared mutable state is required, so handler concurrency is simple.
- The fake service should return cloned values from static seed data to avoid mutation and locking.
- `PORT` parsing should fail fast on invalid configuration.
- The service should keep `/health` independent of poison-pill behavior.

## 12. Implementation Steps

1. Write the plan
   - Change: Add this implementation plan.
   - Files/modules likely affected: `docs/plans/movie-recommendation-api.md`.
   - Notes: Use the project planner structure.
   - Verification: Confirm the plan names requirements, risks, and concrete files.

2. Expand domain types
   - Change: Update `Movie` fields and add recommendation/error response types.
   - Files/modules likely affected: `src/domain/movie.rs`.
   - Notes: Derive `Serialize`, `Deserialize`, `Clone`, and `Debug` where useful for tests.
   - Verification: `cargo test`.

3. Extend service interface and seed data
   - Change: Add recommendation method to `AsyncMovieService`; replace the two-item seed with the sibling ten-movie catalog.
   - Files/modules likely affected: `src/services/movie/movie_service.rs`, `src/services/movie/dummy.rs`.
   - Notes: Keep scores and reasons deterministic.
   - Verification: Unit tests assert seeded titles and recommendation limits.

4. Rework Axum routes
   - Change: Add app state, `/health`, `/movies`, `/recommendations`, query parsing, and `PORT` default `8082`.
   - Files/modules likely affected: `src/main.rs`.
   - Notes: Keep route functions small and explicit.
   - Verification: Handler/router tests for 200 and 503 paths.

5. Add poison-pill handling
   - Change: Parse `X-Demo-Fault` first and `DEMO_FAULT_MODE` as fallback.
   - Files/modules likely affected: `src/main.rs`.
   - Notes: Only `/recommendations` should delay or error.
   - Verification: Tests cover header fault, env fallback, and normal behavior.

6. Add structured logs and Rust telemetry
   - Change: Emit JSON lines on request completion and fault activation.
   - Files/modules likely affected: `src/main.rs`, `Cargo.toml`.
   - Notes: Include `service_name`, `event`, `trace_id`, `correlation_id`, `request_id`, `fault`, `http_route`, `http_status`, and `duration_ms`; add handler spans and custom metrics.
   - Verification: Manual smoke call shows JSON logs on stdout; `cargo test` validates route behavior.

7. Add FastMCP wrapper
   - Change: Add `axum-tools-mcp` Python package with FastMCP tools, Rust API client, telemetry setup, health route, Dockerfile, and README.
   - Files/modules likely affected: `axum-tools-mcp/pyproject.toml`, `axum-tools-mcp/src/axum_tools_mcp/server.py`, `axum-tools-mcp/src/axum_tools_mcp/recommendation_client.py`, `axum-tools-mcp/src/axum_tools_mcp/telemetry.py`, `axum-tools-mcp/Dockerfile`, `axum-tools-mcp/README.md`.
   - Notes: Use official FastMCP `@mcp.tool`, `@mcp.custom_route`, and HTTP transport; propagate `traceparent`, `tracestate`, `X-Correlation-Id`, `X-Request-Id`, and `X-Demo-Fault`.
   - Verification: `uv run python -m compileall src` from `axum-tools-mcp/` and live health/client smoke where dependencies are available.

8. Add Docker support
   - Change: Add Rust API Dockerfile and root Compose file for both services.
   - Files/modules likely affected: `Dockerfile`, `docker-compose.yml`.
   - Notes: Include OTel env vars and Loki labels from the upstream demo contract.
   - Verification: `docker compose config` if Docker Compose is available.

9. Update docs
   - Change: Document routes, env vars, and fault examples.
   - Files/modules likely affected: `README.md`.
   - Notes: Keep commands local and demo-focused.
   - Verification: Commands match implemented routes.

## 13. Testing Strategy

- Unit tests for fake service seed size, title coverage, recommendation limit, and mapping ids.
- Router tests for:
  - `/health` returns 200;
  - `/movies` returns seeded data;
  - `/recommendations` returns the requested limit;
  - `X-Demo-Fault: recommendation-error` returns 503 JSON;
  - `X-Demo-Fault: slow-recommendation` still returns success.
- Verification commands:

```sh
cargo fmt --check
cargo test
cargo check
(cd axum-tools-mcp && uv run python -m compileall src)
docker compose config
```

Optional when installed:

```sh
cargo clippy --all-targets --all-features
```

## 14. Rollout / Migration Plan

- Local rollout only: run the service on `127.0.0.1:8082` or container port `8082`.
- Backward compatibility is low risk because the existing API only exposed a fake `/movies` route without a documented contract.
- Rollback is a git revert of this change set.
- For the full demo, run the Rust API and MCP wrapper together and point the agent at `http://127.0.0.1:8092/mcp`.
- Before connecting the agent, verify normal, slow, and error recommendation behavior.

## 15. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---:|---:|---|
| Full MCP and observability scope increases implementation time | Medium | Medium | Keep MCP thin and avoid live downstream dependencies beyond Rust API |
| Seed data diverges from sibling catalog | Medium | Low | Copy movie ids/titles from the shared fixture and document the source |
| Fault behavior accidentally affects `/health` or `/movies` | High | Low | Implement fault handling only in `/recommendations` and test it |
| Demo cannot correlate logs | Medium | Medium | Preserve/log `traceparent`, `X-Correlation-Id`, and `X-Request-Id` derived fields |
| Future real API call adds another fault source | Medium | Low | Keep this service fake until the demo no longer requires a single fault source |

## 16. Done Criteria

- `docs/plans/movie-recommendation-api.md` exists and is implementation-ready.
- Rust service binds to `8082` by default and honors `PORT`.
- `GET /health` returns a healthy JSON response.
- `GET /movies` returns the seeded catalog.
- `GET /recommendations?limit=5` returns deterministic recommendation data.
- Recommendation ordering uses a deterministic heuristic based on preference, rating, runtime, seeded popularity, and screening availability hints.
- Recommendation seed data includes the sibling demo movie catalog and reservation movie ids.
- `X-Demo-Fault: slow-recommendation` delays and then succeeds.
- `X-Demo-Fault: recommendation-error` returns HTTP 503 with JSON error.
- Rust service emits custom traces and metrics when OTel export is configured.
- FastMCP wrapper runs on `8092`, exposes `/health`, and defines `recommendation_get_movies` and `recommendation_health`.
- `cargo fmt --check` and `cargo test` pass.

## 17. Review Checklist

- [x] Requirements are explicit
- [x] Non-goals are explicit
- [x] Existing code conventions were checked
- [x] Alternatives were considered
- [x] Security implications were reviewed
- [x] Scalability and reliability implications were reviewed
- [x] Testing strategy is complete
- [x] Rollout and rollback are defined
- [x] Implementation steps are ordered and concrete

## 18. Handoff Prompt for Implementation Agent

Copy/paste this prompt into a coding agent:

```text
Implement the plan in docs/plans/movie-recommendation-api.md.

Constraints:
- Keep the first slice focused on the Rust Axum API.
- Do not call external movie APIs.
- Preserve the poison-pill contract exactly: none, slow-recommendation, recommendation-error.
- Seed the fake catalog from the sibling golden-path movie reservation fixture.
- Implement deterministic heuristic ranking.
- Clamp caller-provided limits.
- Emit structured stdout logs with service, event, ids, fault, route, status, and duration.
- Add Rust OpenTelemetry spans and custom metrics.
- Implement the FastMCP wrapper in axum-tools-mcp.
- Update tests and README.

Relevant files/modules:
- src/main.rs
- src/domain/movie.rs
- src/services/movie/movie_service.rs
- src/services/movie/dummy.rs
- src/di/movie_service.rs
- axum-tools-mcp/**
- Dockerfile
- docker-compose.yml
- README.md

Expected verification commands:
- cargo fmt --check
- cargo test
- cargo check
- (cd axum-tools-mcp && uv run python -m compileall src)
- docker compose config
```
