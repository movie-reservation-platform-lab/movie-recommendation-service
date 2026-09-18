use crate::audit::config::EventIdentity;
use anyhow::{Context as _, Result};
use opentelemetry::{
    global,
    metrics::{Counter, Histogram, Meter, MeterProvider as _},
    KeyValue,
};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    metrics::SdkMeterProvider, propagation::TraceContextPropagator, trace::SdkTracerProvider,
    Resource,
};
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
        Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tracing::warn;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub struct Telemetry {
    service_name: &'static str,
    service_version: String,
    deployment_environment: String,
    metrics: TelemetryMetrics,
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: SdkMeterProvider,
    event_logger: Option<EventLogger>,
}

const EVENT_BUFFER_CAPACITY: usize = 1024;

struct EventLogger {
    sender: Mutex<Option<SyncSender<serde_json::Value>>>,
    completion: Mutex<Option<Receiver<()>>>,
    dropped_events: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HttpEventKind {
    HealthCompleted,
    ReadinessCompleted,
    MoviesCompleted,
    RecommendationsCompleted,
    RequestRejected,
    RequestFailed,
}

impl HttpEventKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::HealthCompleted => "health.completed",
            Self::ReadinessCompleted => "readiness.completed",
            Self::MoviesCompleted => "movies.completed",
            Self::RecommendationsCompleted => "recommendations.completed",
            Self::RequestRejected => "request.rejected",
            Self::RequestFailed => "request.failed",
        }
    }
}

pub(crate) struct HttpRequestEvent<'a> {
    pub(crate) kind: HttpEventKind,
    pub(crate) trace_id: Option<&'a str>,
    pub(crate) span_id: Option<&'a str>,
    pub(crate) correlation_id: &'a str,
    pub(crate) request_id: &'a str,
    pub(crate) route: &'static str,
    pub(crate) status: u16,
    pub(crate) duration_ms: u64,
}

#[derive(Clone)]
struct TelemetryMetrics {
    http_requests: Counter<u64>,
    http_request_duration: Histogram<f64>,
    recommendations: Counter<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HttpMetricLabels {
    route: &'static str,
    status: u16,
}

impl HttpMetricLabels {
    fn attributes(self) -> [KeyValue; 4] {
        [
            KeyValue::new("http.route", self.route),
            KeyValue::new("http.status_code", i64::from(self.status)),
            KeyValue::new("http.status_class", status_class(self.status)),
            KeyValue::new("outcome", request_outcome(self.status)),
        ]
    }
}

const fn status_class(status: u16) -> &'static str {
    match status {
        200..=299 => "2xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "other",
    }
}

const fn request_outcome(status: u16) -> &'static str {
    match status {
        200..=399 => "success",
        400..=499 => "client_error",
        _ => "server_error",
    }
}

impl EventLogger {
    fn new(capacity: usize) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let (completion_sender, completion) = mpsc::channel();

        thread::Builder::new()
            .name("application-event-logger".into())
            .spawn(move || {
                for event in receiver {
                    let stdout = io::stdout();
                    write_json_line(stdout.lock(), &event);
                }
                let _ = completion_sender.send(());
            })?;

        Ok(Self {
            sender: Mutex::new(Some(sender)),
            completion: Mutex::new(Some(completion)),
            dropped_events: AtomicU64::new(0),
        })
    }

    fn emit(&self, event: serde_json::Value) {
        let result = self
            .sender
            .try_lock()
            .ok()
            .and_then(|sender| sender.as_ref().map(|sender| sender.try_send(event)));

        if !matches!(result, Some(Ok(()))) {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        if let Ok(mut sender) = self.sender.lock() {
            sender.take();
        }

        let completed = self
            .completion
            .lock()
            .ok()
            .and_then(|mut completion| completion.take())
            .is_none_or(|completion| completion.recv_timeout(timeout).is_ok());
        let dropped_events = self.dropped_events.load(Ordering::Relaxed);
        if dropped_events > 0 {
            warn!(dropped_events, "application events were dropped");
        }

        completed
    }
}

impl Telemetry {
    #[cfg(test)]
    pub fn noop(service_name: &'static str) -> Self {
        let meter_provider = SdkMeterProvider::builder().build();
        Self {
            service_name,
            service_version: "test".into(),
            deployment_environment: "test".into(),
            metrics: build_metrics(meter_provider.meter(service_name)),
            tracer_provider: None,
            meter_provider,
            event_logger: None,
        }
    }

    pub(crate) fn record_http_event(&self, event: HttpRequestEvent<'_>) {
        let attributes = HttpMetricLabels {
            route: event.route,
            status: event.status,
        }
        .attributes();

        self.metrics.http_requests.add(1, &attributes);
        self.metrics
            .http_request_duration
            .record(event.duration_ms as f64, &attributes);

        self.emit_event(http_event_json(
            self.service_name,
            &self.service_version,
            &self.deployment_environment,
            event,
        ));
    }

    pub fn record_recommendations(&self, count: usize, preference_present: bool) {
        self.metrics.recommendations.add(
            count as u64,
            &[KeyValue::new("preference.present", preference_present)],
        );
    }

    pub(crate) fn record_starting(&self, address: &str, port: u16, movie_provider: &'static str) {
        self.emit_event(serde_json::json!({
            "service_name": self.service_name,
            "event": "service.starting",
            "address": address,
            "port": port,
            "movie_provider": movie_provider
        }));
    }

    pub(crate) fn record_shutdown_requested(&self, reason: &'static str) {
        self.emit_event(serde_json::json!({
            "service_name": self.service_name,
            "event": "service.shutdown_requested",
            "reason": reason
        }));
    }

    fn emit_event(&self, event: serde_json::Value) {
        if let Some(event_logger) = &self.event_logger {
            event_logger.emit(event);
        }
    }

    pub fn shutdown(&self, timeout: Duration) {
        let deadline = Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now);
        if let Some(event_logger) = &self.event_logger {
            let completed = event_logger.shutdown(remaining_until(deadline));
            if !completed {
                warn!("application event logger shutdown deadline expired");
            }
        }

        let (sender, receiver) = mpsc::channel();
        let mut worker_count = 0;

        let meter_provider = self.meter_provider.clone();
        let meter_sender = sender.clone();
        match thread::Builder::new()
            .name("otel-meter-shutdown".into())
            .spawn(move || {
                if let Err(error) = meter_provider.shutdown() {
                    warn!(error = %error, "failed to shutdown OpenTelemetry meter provider");
                }
                let _ = meter_sender.send(());
            }) {
            Ok(_) => worker_count += 1,
            Err(error) => {
                warn!(error = %error, "failed to start OpenTelemetry meter shutdown worker");
            }
        }

        if let Some(tracer_provider) = &self.tracer_provider {
            let tracer_provider = tracer_provider.clone();
            let sender = sender.clone();
            match thread::Builder::new()
                .name("otel-tracer-shutdown".into())
                .spawn(move || {
                    if let Err(error) = tracer_provider.shutdown() {
                        warn!(error = %error, "failed to shutdown OpenTelemetry tracer provider");
                    }
                    let _ = sender.send(());
                }) {
                Ok(_) => worker_count += 1,
                Err(error) => {
                    warn!(error = %error, "failed to start OpenTelemetry tracer shutdown worker");
                }
            }
        }

        drop(sender);
        let completed =
            wait_for_shutdown_workers(&receiver, worker_count, remaining_until(deadline));
        if completed < worker_count {
            warn!(
                completed,
                pending = worker_count - completed,
                timeout_ms = timeout.as_millis(),
                "OpenTelemetry shutdown deadline expired"
            );
        }
    }
}

pub fn init(
    service_name: &'static str,
    otlp_endpoint: Option<&str>,
    identity: &EventIdentity,
) -> Result<Telemetry> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer_provider = build_tracer_provider(service_name, otlp_endpoint, identity);
    if let Some(provider) = &tracer_provider {
        global::set_tracer_provider(provider.clone());
    }

    let meter_provider = build_meter_provider(service_name, otlp_endpoint, identity);
    global::set_meter_provider(meter_provider.clone());

    init_tracing_subscriber(service_name, tracer_provider.as_ref())?;
    let event_logger = EventLogger::new(EVENT_BUFFER_CAPACITY)
        .context("failed to initialize application event logger")?;

    Ok(Telemetry {
        service_name,
        service_version: identity.version.clone(),
        deployment_environment: identity.environment.clone(),
        metrics: build_metrics(meter_provider.meter(service_name)),
        tracer_provider,
        meter_provider,
        event_logger: Some(event_logger),
    })
}

fn http_event_json(
    service_name: &'static str,
    service_version: &str,
    deployment_environment: &str,
    event: HttpRequestEvent<'_>,
) -> serde_json::Value {
    let timestamp_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let severity = match event.status {
        500..=599 => "ERROR",
        400..=499 => "WARN",
        _ => "INFO",
    };
    let mut value = serde_json::json!({
        "timestamp_unix_ms": timestamp_unix_ms,
        "severity_text": severity,
        "service_name": service_name,
        "service_version": service_version,
        "deployment_environment": deployment_environment,
        "event": event.kind.as_str(),
        "correlation_id": event.correlation_id,
        "request_id": event.request_id,
        "http_route": event.route,
        "http_status": event.status,
        "duration_ms": event.duration_ms
    });
    if let Some(trace_id) = event.trace_id {
        value["trace_id"] = serde_json::Value::String(trace_id.into());
    }
    if let Some(span_id) = event.span_id {
        value["span_id"] = serde_json::Value::String(span_id.into());
    }
    value
}

fn emit_json_error(event: serde_json::Value) {
    let stderr = io::stderr();
    write_json_line(stderr.lock(), &event);
}

fn write_json_line(mut writer: impl Write, event: &serde_json::Value) {
    if serde_json::to_writer(&mut writer, event).is_ok() {
        let _ = writer.write_all(b"\n");
    }
}

fn init_tracing_subscriber(
    service_name: &'static str,
    tracer_provider: Option<&SdkTracerProvider>,
) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_target(false)
        .with_filter(env_filter);
    let otel_layer = tracer_provider.map(|provider| {
        use opentelemetry::trace::TracerProvider as _;

        OpenTelemetryLayer::new(provider.tracer(service_name))
    });

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()
        .context("failed to initialize tracing subscriber")
}

fn wait_for_shutdown_workers(
    receiver: &Receiver<()>,
    worker_count: usize,
    timeout: Duration,
) -> usize {
    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    let mut completed = 0;

    while completed < worker_count {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining) {
            Ok(()) => completed += 1,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }

    completed
}

fn remaining_until(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn build_tracer_provider(
    service_name: &'static str,
    otlp_endpoint: Option<&str>,
    identity: &EventIdentity,
) -> Option<SdkTracerProvider> {
    let otlp_endpoint = otlp_endpoint?;
    let trace_endpoint = signal_endpoint(otlp_endpoint, "v1/traces");

    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(trace_endpoint)
        .build()
    {
        Ok(exporter) => exporter,
        Err(_) => {
            emit_json_error(serde_json::json!({
                "service_name": service_name,
                "event": "otel.trace_exporter.disabled",
                "reason": "exporter_configuration_invalid"
            }));
            return None;
        }
    };

    Some(
        SdkTracerProvider::builder()
            .with_resource(resource(service_name, identity))
            .with_batch_exporter(exporter)
            .build(),
    )
}

fn build_meter_provider(
    service_name: &'static str,
    otlp_endpoint: Option<&str>,
    identity: &EventIdentity,
) -> SdkMeterProvider {
    let Some(otlp_endpoint) = otlp_endpoint else {
        return SdkMeterProvider::builder()
            .with_resource(resource(service_name, identity))
            .build();
    };
    let metric_endpoint = signal_endpoint(otlp_endpoint, "v1/metrics");

    let exporter = match opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(metric_endpoint)
        .build()
    {
        Ok(exporter) => exporter,
        Err(_) => {
            emit_json_error(serde_json::json!({
                "service_name": service_name,
                "event": "otel.metric_exporter.disabled",
                "reason": "exporter_configuration_invalid"
            }));
            return SdkMeterProvider::builder()
                .with_resource(resource(service_name, identity))
                .build();
        }
    };

    SdkMeterProvider::builder()
        .with_resource(resource(service_name, identity))
        .with_periodic_exporter(exporter)
        .build()
}

fn signal_endpoint(base_endpoint: &str, signal_path: &str) -> String {
    format!("{}/{signal_path}", base_endpoint.trim_end_matches('/'))
}

fn resource(service_name: &'static str, identity: &EventIdentity) -> Resource {
    Resource::builder()
        .with_service_name(service_name)
        .with_attribute(KeyValue::new("service.version", identity.version.clone()))
        .with_attribute(KeyValue::new(
            "deployment.environment.name",
            identity.environment.clone(),
        ))
        .with_attribute(KeyValue::new(
            "deployment.environment",
            identity.environment.clone(),
        ))
        .with_attribute(KeyValue::new("demo.name", "multi-service-observability"))
        .build()
}

fn build_metrics(meter: Meter) -> TelemetryMetrics {
    TelemetryMetrics {
        http_requests: meter
            .u64_counter("movie_recommendation_service_http_requests_total")
            .with_description("Total inbound HTTP requests handled by route, status, and outcome.")
            .build(),
        http_request_duration: meter
            .f64_histogram("movie_recommendation_service_http_request_duration_ms")
            .with_description("Inbound HTTP request duration in milliseconds.")
            .with_unit("ms")
            .build(),
        recommendations: meter
            .u64_counter("movie_recommendation_service_recommendations_total")
            .with_description("Total recommendation items returned to callers.")
            .build(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_share_audit_artifact_and_environment_identity() {
        let identity = EventIdentity::from_values(Some("build-123"), Some("audit-demo")).unwrap();
        let resource = resource(crate::SERVICE_NAME, &identity);
        for (key, value) in [
            ("service.name", crate::SERVICE_NAME),
            ("service.version", "build-123"),
            ("deployment.environment.name", "audit-demo"),
            ("deployment.environment", "audit-demo"),
        ] {
            assert_eq!(
                resource.get(&opentelemetry::Key::from_static_str(key)),
                Some(opentelemetry::Value::from(value))
            );
        }
    }

    #[test]
    fn http_metric_labels_are_bounded_contract_values() {
        let labels = HttpMetricLabels {
            route: "/recommendations",
            status: 503,
        };

        assert_eq!(labels.route, "/recommendations");
        assert_eq!(labels.status, 503);
        assert_eq!(labels.attributes().len(), 4);
        assert_eq!(status_class(503), "5xx");
        assert_eq!(request_outcome(503), "server_error");
    }

    #[test]
    fn http_event_kinds_are_closed_contract_values() {
        let cases = [
            (HttpEventKind::HealthCompleted, "health.completed"),
            (HttpEventKind::ReadinessCompleted, "readiness.completed"),
            (HttpEventKind::MoviesCompleted, "movies.completed"),
            (
                HttpEventKind::RecommendationsCompleted,
                "recommendations.completed",
            ),
            (HttpEventKind::RequestRejected, "request.rejected"),
            (HttpEventKind::RequestFailed, "request.failed"),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind.as_str(), expected);
        }
    }

    #[test]
    fn application_event_is_one_json_line() {
        let event = serde_json::json!({"event": "health.completed"});
        let mut output = Vec::new();

        write_json_line(&mut output, &event);

        assert_eq!(output, b"{\"event\":\"health.completed\"}\n");
    }

    #[test]
    fn request_event_contains_resource_and_real_span_correlation() {
        let event = http_event_json(
            crate::SERVICE_NAME,
            "build-123",
            "test",
            HttpRequestEvent {
                kind: HttpEventKind::RequestFailed,
                trace_id: Some("4bf92f3577b34da6a3ce929d0e0e4736"),
                span_id: Some("00f067aa0ba902b7"),
                correlation_id: "correlation-123",
                request_id: "request-123",
                route: "/recommendations",
                status: 500,
                duration_ms: 12,
            },
        );

        assert_eq!(event["severity_text"], "ERROR");
        assert_eq!(event["service_name"], crate::SERVICE_NAME);
        assert_eq!(event["service_version"], "build-123");
        assert_eq!(event["deployment_environment"], "test");
        assert_eq!(event["trace_id"], "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(event["span_id"], "00f067aa0ba902b7");
        assert!(event["timestamp_unix_ms"].is_number());
    }

    #[test]
    fn request_event_omits_trace_fields_without_an_active_span() {
        let event = http_event_json(
            crate::SERVICE_NAME,
            "build-123",
            "test",
            HttpRequestEvent {
                kind: HttpEventKind::RequestRejected,
                trace_id: None,
                span_id: None,
                correlation_id: "correlation-123",
                request_id: "request-123",
                route: "/recommendations",
                status: 400,
                duration_ms: 1,
            },
        );

        assert_eq!(event["severity_text"], "WARN");
        assert!(event.get("trace_id").is_none());
        assert!(event.get("span_id").is_none());
    }

    #[test]
    fn exported_http_metrics_prove_exact_resource_series_and_aggregation() {
        use opentelemetry_sdk::metrics::{
            data::{AggregatedMetrics, MetricData},
            InMemoryMetricExporter,
        };

        let identity = EventIdentity::from_values(Some("build-123"), Some("test")).unwrap();
        let exporter = InMemoryMetricExporter::default();
        let meter_provider = SdkMeterProvider::builder()
            .with_resource(resource(crate::SERVICE_NAME, &identity))
            .with_periodic_exporter(exporter.clone())
            .build();
        let telemetry = Telemetry {
            service_name: crate::SERVICE_NAME,
            service_version: identity.version.clone(),
            deployment_environment: identity.environment.clone(),
            metrics: build_metrics(meter_provider.meter(crate::SERVICE_NAME)),
            tracer_provider: None,
            meter_provider,
            event_logger: None,
        };

        for (kind, status) in [
            (HttpEventKind::RecommendationsCompleted, 200),
            (HttpEventKind::RequestRejected, 400),
            (HttpEventKind::RequestFailed, 500),
        ] {
            telemetry.record_http_event(HttpRequestEvent {
                kind,
                trace_id: None,
                span_id: None,
                correlation_id: "not-a-metric-label",
                request_id: "not-a-metric-label",
                route: "/recommendations",
                status,
                duration_ms: u64::from(status / 100),
            });
        }
        telemetry.meter_provider.force_flush().unwrap();

        let exports = exporter.get_finished_metrics().unwrap();
        let payload = exports.last().unwrap();
        for (key, expected) in [
            ("service.name", crate::SERVICE_NAME),
            ("service.version", "build-123"),
            ("deployment.environment.name", "test"),
        ] {
            assert_eq!(
                payload
                    .resource()
                    .get(&opentelemetry::Key::from_static_str(key)),
                Some(opentelemetry::Value::from(expected))
            );
        }
        let metrics = payload
            .scope_metrics()
            .flat_map(|scope| scope.metrics())
            .collect::<Vec<_>>();
        let requests = metrics
            .iter()
            .find(|metric| metric.name() == "movie_recommendation_service_http_requests_total")
            .unwrap();
        let durations = metrics
            .iter()
            .find(|metric| metric.name() == "movie_recommendation_service_http_request_duration_ms")
            .unwrap();
        assert_eq!(requests.unit(), "");
        assert_eq!(durations.unit(), "ms");

        let AggregatedMetrics::U64(MetricData::Sum(request_sum)) = requests.data() else {
            panic!("request counter must export a u64 sum");
        };
        assert!(request_sum.is_monotonic());
        assert_eq!(
            request_sum.temporality(),
            opentelemetry_sdk::metrics::Temporality::Cumulative
        );
        let points = request_sum.data_points().collect::<Vec<_>>();
        assert_eq!(points.len(), 3);
        assert!(points.iter().all(|point| point.value() == 1));
        for (status, class, outcome) in [
            (200_i64, "2xx", "success"),
            (400_i64, "4xx", "client_error"),
            (500_i64, "5xx", "server_error"),
        ] {
            assert!(points.iter().any(|point| {
                let attributes = point
                    .attributes()
                    .map(|item| (item.key.as_str(), item.value.to_string()))
                    .collect::<Vec<_>>();
                attributes.contains(&("http.route", "/recommendations".into()))
                    && attributes.contains(&("http.status_code", status.to_string()))
                    && attributes.contains(&("http.status_class", class.into()))
                    && attributes.contains(&("outcome", outcome.into()))
                    && !attributes.iter().any(|(key, _)| {
                        matches!(
                            *key,
                            "request_id" | "correlation_id" | "trace_id" | "demo.fault"
                        )
                    })
            }));
        }

        let AggregatedMetrics::F64(MetricData::Histogram(duration_histogram)) = durations.data()
        else {
            panic!("request duration must export an f64 histogram");
        };
        assert_eq!(duration_histogram.data_points().count(), 3);
        assert!(duration_histogram
            .data_points()
            .all(|point| point.count() == 1));
    }

    #[test]
    fn full_event_buffer_drops_without_waiting() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let (_completion_sender, completion) = mpsc::channel();
        let logger = EventLogger {
            sender: Mutex::new(Some(sender)),
            completion: Mutex::new(Some(completion)),
            dropped_events: AtomicU64::new(0),
        };

        logger.emit(serde_json::json!({"event": "first"}));
        logger.emit(serde_json::json!({"event": "second"}));

        assert_eq!(logger.dropped_events.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn shutdown_worker_wait_completes_when_all_workers_report() {
        let (sender, receiver) = mpsc::channel();
        sender.send(()).unwrap();
        sender.send(()).unwrap();

        assert_eq!(
            wait_for_shutdown_workers(&receiver, 2, Duration::from_millis(10)),
            2
        );
    }

    #[test]
    fn shutdown_worker_wait_obeys_zero_deadline() {
        let (_sender, receiver) = mpsc::channel::<()>();

        assert_eq!(wait_for_shutdown_workers(&receiver, 1, Duration::ZERO), 0);
    }
}
