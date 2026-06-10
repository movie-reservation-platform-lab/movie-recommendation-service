# axum-tools-mcp

FastMCP wrapper around the Rust Axum recommendation API.

## Run

```sh
uv run axum-tools-mcp
```

Defaults:

- MCP endpoint: `http://127.0.0.1:8092/mcp`
- Health endpoint: `http://127.0.0.1:8092/health`
- Downstream API: `http://127.0.0.1:8082`

Useful environment variables:

- `AXUM_TOOLS_API_URL`
- `PORT`
- `HOST`
- `OTEL_SERVICE_NAME`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL`
- `OTEL_RESOURCE_ATTRIBUTES`

## Tools

- `recommendation_get_movies`
  - Inputs: `limit`, optional `preference`, optional `fault`, and optional propagation fields.
  - Calls Rust `GET /recommendations`.
- `recommendation_health`
  - Calls Rust `GET /health`.

Both tools forward:

- `traceparent`
- `tracestate`
- `X-Correlation-Id`
- `X-Request-Id`
- `X-Demo-Fault`

## Checks

```sh
uv run python -m compileall src
```
