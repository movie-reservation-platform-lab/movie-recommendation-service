from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

import httpx

DEFAULT_API_URL = "http://127.0.0.1:8082"
PROPAGATED_HEADERS = {
    "traceparent": "traceparent",
    "tracestate": "tracestate",
    "correlation_id": "X-Correlation-Id",
    "request_id": "X-Request-Id",
    "demo_fault": "X-Demo-Fault",
}


class RecommendationClientError(RuntimeError):
    def __init__(self, status_code: int, payload: dict[str, Any]) -> None:
        self.status_code = status_code
        self.payload = payload
        super().__init__(f"Recommendation API returned HTTP {status_code}")


@dataclass(frozen=True)
class RequestMetadata:
    traceparent: str | None = None
    tracestate: str | None = None
    correlation_id: str | None = None
    request_id: str | None = None
    demo_fault: str | None = None

    def headers(self) -> dict[str, str]:
        values = {
            "traceparent": self.traceparent,
            "tracestate": self.tracestate,
            "correlation_id": self.correlation_id,
            "request_id": self.request_id,
            "demo_fault": self.demo_fault,
        }
        return {
            header_name: value
            for field_name, header_name in PROPAGATED_HEADERS.items()
            if (value := values[field_name]) is not None and value.strip()
        }


class RecommendationClient:
    def __init__(self, base_url: str | None = None, timeout_seconds: float = 10.0) -> None:
        self._base_url = (base_url or os.getenv("AXUM_TOOLS_API_URL") or DEFAULT_API_URL).rstrip("/")
        self._client = httpx.AsyncClient(base_url=self._base_url, timeout=timeout_seconds)

    async def close(self) -> None:
        await self._client.aclose()

    async def health(self, metadata: RequestMetadata) -> dict[str, Any]:
        response = await self._client.get("/health", headers=metadata.headers())
        return response.json()

    async def recommendations(
        self,
        *,
        limit: int,
        preference: str | None,
        metadata: RequestMetadata,
    ) -> dict[str, Any]:
        params: dict[str, Any] = {"limit": limit}
        if preference is not None and preference.strip():
            params["preference"] = preference

        response = await self._client.get("/recommendations", params=params, headers=metadata.headers())
        payload = response.json()
        if response.status_code >= 400:
            raise RecommendationClientError(response.status_code, payload)

        return payload
