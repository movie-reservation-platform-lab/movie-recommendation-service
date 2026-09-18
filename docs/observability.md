# Recommendation service observability

Tracking: [issue #2](https://github.com/movie-reservation-platform-lab/movie-recommendation-service/issues/2).
This records producer-native output for the advisory
`movie-platform-infra#57` signal contract. Backend translation and live delivery
are separate infrastructure acceptance stages.

## Resource identity

The OpenTelemetry SDK resource includes:

- `service.name=movie-recommendation-service`;
- `service.version` from `SERVICE_VERSION`;
- `deployment.environment.name` from `DEPLOYMENT_ENVIRONMENT`;
- `service.namespace` from the platform-provided `OTEL_RESOURCE_ATTRIBUTES`.

The service supplies safe local defaults for version and environment. The
platform owns the namespace and selected immutable version.

## Observed native metrics

Credential-free in-memory exporter tests prove this payload before backend
translation:

| Instrument | SDK data | Unit | Attributes |
| --- | --- | --- | --- |
| `movie_recommendation_service_http_requests_total` | monotonic `u64` sum, cumulative | none | `http.route`, `http.status_code`, `http.status_class`, `outcome` |
| `movie_recommendation_service_http_request_duration_ms` | `f64` histogram, cumulative | `ms` | same HTTP attributes |
| `movie_recommendation_service_recommendations_total` | monotonic `u64` sum, cumulative | none | `preference.present` |

The fixed HTTP values are:

- routes: `/health`, `/ready`, `/movies`, `/recommendations`;
- status classes: `2xx`, `4xx`, `5xx`, `other`;
- outcomes: `success`, `client_error`, `server_error`.

Request, correlation, trace and span IDs are never metric attributes. Health
and readiness are observable but must be excluded from user-path queries by
their fixed route. The default periodic export cadence is 60 seconds and may be
overridden by `OTEL_METRIC_EXPORT_INTERVAL`.

Counters and histograms appear only after an eligible observation. The service
does not invent requests, error zeros or duration observations. No series during
an idle interval is different from a fresh measured zero; stale/missing export
must therefore be checked independently before treating an error-rate result as
healthy.

## Traces and logs

Inbound W3C context is extracted before the `http.request` server span. The
`recommendations.rank` span is its child and both are marked failed when ranking
returns a non-finite score. This is covered with an in-memory span exporter.

Each request also emits a JSON stdout event containing timestamp, severity,
service name/version, deployment environment, fixed event name, route, status,
duration, request ID and correlation ID. `trace_id` and `span_id` are included
only when a valid active span exists. Ranking failures emit the bounded
`error.type=non_finite_score`; raw preferences, bodies and internal error text
are not copied into metric labels or public errors.

## Local verification

```sh
cargo test telemetry::tests --locked
cargo test ranking_failure_exports_correlated_error_spans --locked
cargo test request_metadata_cannot_select_or_bypass_ranking_outcomes --locked
```

The infrastructure owner must still verify the translated AMP/CloudWatch names,
collector routing, X-Ray/Tempo delivery and live freshness after selecting the
reviewed image digest.
