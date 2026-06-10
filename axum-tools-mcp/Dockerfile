# syntax=docker/dockerfile:1.7

ARG UV_IMAGE=ghcr.io/astral-sh/uv:python3.12-bookworm-slim
FROM ${UV_IMAGE} AS base

ENV PYTHONUNBUFFERED=1 \
    UV_LINK_MODE=copy \
    UV_PROJECT_ENVIRONMENT=/venv \
    VIRTUAL_ENV=/venv \
    PATH=/venv/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl tini \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

FROM base AS dependencies
COPY pyproject.toml README.md uv.lock /app/
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --no-install-project --no-dev --frozen

FROM dependencies AS build
COPY src /app/src
RUN uv sync --no-dev --compile-bytecode --no-editable

FROM base AS prod
COPY --from=build /venv /venv
COPY --from=build /app /app

RUN useradd -r -u 10001 appuser && chown -R appuser:appuser /app /venv
USER appuser

EXPOSE 8092
HEALTHCHECK --interval=30s --timeout=3s --retries=3 CMD \
  sh -c 'curl -fsS http://127.0.0.1:8092/health || exit 1'

ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["axum-tools-mcp"]
