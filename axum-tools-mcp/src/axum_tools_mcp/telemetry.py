from __future__ import annotations

import json
import logging
import os
import sys
import time
from collections.abc import Mapping
from contextlib import contextmanager
from typing import Any

from opentelemetry import metrics, trace
from opentelemetry.exporter.otlp.proto.http.metric_exporter import OTLPMetricExporter
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.instrumentation.httpx import HTTPXClientInstrumentor
from opentelemetry.sdk.metrics import MeterProvider
from opentelemetry.sdk.metrics.export import PeriodicExportingMetricReader
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.trace import SpanKind, Status, StatusCode

SERVICE_NAME = "axum-tools-mcp"

_tracer = trace.get_tracer(SERVICE_NAME)
_meter = metrics.get_meter(SERVICE_NAME)
_tool_calls = _meter.create_counter(
    "axum_tools_mcp_tool_calls_total",
    description="Total MCP tool calls by tool and outcome.",
)
_tool_duration = _meter.create_histogram(
    "axum_tools_mcp_tool_duration_ms",
    unit="ms",
    description="MCP tool call duration in milliseconds.",
)


class JsonFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        payload = {
            "service_name": SERVICE_NAME,
            "event": getattr(record, "event", record.getMessage()),
            "level": record.levelname.lower(),
            "message": record.getMessage(),
        }

        for key, value in getattr(record, "fields", {}).items():
            if value is not None:
                payload[key] = value

        return json.dumps(payload, sort_keys=True, separators=(",", ":"))


def configure_telemetry() -> None:
    resource = Resource.create(
        {
            "service.name": os.getenv("OTEL_SERVICE_NAME", SERVICE_NAME),
            "service.environment": "local",
            "demo.name": "multi-service-observability",
        }
    )

    if os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT"):
        tracer_provider = TracerProvider(resource=resource)
        tracer_provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
        trace.set_tracer_provider(tracer_provider)

        metric_reader = PeriodicExportingMetricReader(OTLPMetricExporter())
        metrics.set_meter_provider(MeterProvider(resource=resource, metric_readers=[metric_reader]))

    HTTPXClientInstrumentor().instrument()
    configure_logging()


def configure_logging() -> None:
    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(JsonFormatter())

    root_logger = logging.getLogger()
    root_logger.handlers.clear()
    root_logger.addHandler(handler)
    root_logger.setLevel(os.getenv("LOG_LEVEL", "INFO").upper())


def get_logger(name: str) -> logging.Logger:
    return logging.getLogger(name)


@contextmanager
def tool_span(tool_name: str, fields: Mapping[str, Any]):
    started_at = time.perf_counter()
    with trace.get_tracer(SERVICE_NAME).start_as_current_span(
        f"mcp.tool.{tool_name}",
        kind=SpanKind.SERVER,
        attributes={
            "mcp.tool.name": tool_name,
            "demo.fault": fields.get("fault") or "none",
        },
    ) as span:
        try:
            yield span
        except Exception as exc:
            span.record_exception(exc)
            span.set_status(Status(StatusCode.ERROR, str(exc)))
            record_tool_metrics(tool_name, "error", fields.get("fault"), started_at)
            raise
        else:
            span.set_status(Status(StatusCode.OK))
            record_tool_metrics(tool_name, "success", fields.get("fault"), started_at)


def record_tool_metrics(tool_name: str, outcome: str, fault: object, started_at: float) -> None:
    duration_ms = (time.perf_counter() - started_at) * 1000
    attributes = {
        "mcp.tool.name": tool_name,
        "outcome": outcome,
        "demo.fault": str(fault or "none"),
    }
    _tool_calls.add(1, attributes)
    _tool_duration.record(duration_ms, attributes)


def log_event(logger: logging.Logger, event: str, message: str, **fields: Any) -> None:
    logger.info(message, extra={"event": event, "fields": fields})
