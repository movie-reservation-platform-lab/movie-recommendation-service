# axum_tools_random_api

Rust/Axum demo recommendation API for the multi-service observability demo.

## Run

```sh
USE_DUMMY=true PORT=8082 cargo run
```

The service defaults to `PORT=8082` and fake movie data.

## Routes

- `GET /health`
- `GET /movies?limit=10`
- `GET /recommendations?limit=5&preference=sci-fi`

`/recommendations` returns deterministic recommendations seeded from the
`golden-path-ecs-template` movie reservation demo catalog. Recommendation items
include `movie_reservation_movie_id` so agent flows can map recommendation
context to the reservation service.

## Demo Faults

Faults are only applied to `GET /recommendations`.

Use `X-Demo-Fault` first:

```sh
curl -H 'X-Demo-Fault: slow-recommendation' \
  'http://127.0.0.1:8082/recommendations?limit=2'
```

Supported values:

- `none`
- `slow-recommendation`
- `recommendation-error`

If the header is absent, `DEMO_FAULT_MODE` is used as a fallback.

## Checks

```sh
cargo fmt --check
cargo test
cargo check
```

## FastMCP Wrapper

The Python MCP wrapper lives in `axum-tools-mcp/` and uses `uv`.

```sh
cd axum-tools-mcp
uv run axum-tools-mcp
```

It serves:

- `GET /health` on `http://127.0.0.1:8092/health`
- MCP HTTP transport on `http://127.0.0.1:8092/mcp`

Python check:

```sh
cd axum-tools-mcp
uv run python -m compileall src
```
