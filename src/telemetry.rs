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
    time::{Duration, Instant},
};
use tracing::warn;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub struct Telemetry {
    service_name: &'static str,
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
    RecommendationsFaultError,
    RecommendationsFaultDelayCompleted,
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
            Self::RecommendationsFaultError => "recommendations.fault_error",
            Self::RecommendationsFaultDelayCompleted => "recommendations.fault_delay_completed",
            Self::RequestRejected => "request.rejected",
            Self::RequestFailed => "request.failed",
        }
    }
}

pub(crate) struct HttpRequestEvent<'a> {
    pub(crate) kind: HttpEventKind,
    pub(crate) trace_id: &'a str,
    pub(crate) correlation_id: &'a str,
    pub(crate) request_id: &'a str,
    pub(crate) fault: &'static str,
    pub(crate) route: &'static str,
    pub(crate) status: u16,
    pub(crate) duration_ms: u64,
}

#[derive(Clone)]
struct TelemetryMetrics {
    http_requests: Counter<u64>,
    http_request_duration: Histogram<f64>,
    recommendations: Counter<u64>,
    faults: Counter<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HttpMetricLabels {
    route: &'static str,
    status: u16,
    fault: &'static str,
}

impl HttpMetricLabels {
    fn attributes(self) -> [KeyValue; 3] {
        [
            KeyValue::new("http.route", self.route),
            KeyValue::new("http.status_code", i64::from(self.status)),
            KeyValue::new("demo.fault", self.fault),
        ]
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
            fault: event.fault,
        }
        .attributes();

        self.metrics.http_requests.add(1, &attributes);
        self.metrics
            .http_request_duration
            .record(event.duration_ms as f64, &attributes);

        self.emit_event(serde_json::json!({
            "service_name": self.service_name,
            "event": event.kind.as_str(),
            "trace_id": event.trace_id,
            "correlation_id": event.correlation_id,
            "request_id": event.request_id,
            "fault": event.fault,
            "http_route": event.route,
            "http_status": event.status,
            "duration_ms": event.duration_ms
        }));
    }

    pub fn record_recommendations(&self, count: usize, preference_present: bool) {
        self.metrics.recommendations.add(
            count as u64,
            &[KeyValue::new("preference.present", preference_present)],
        );
    }

    pub fn record_fault(&self, fault: &'static str) {
        self.metrics
            .faults
            .add(1, &[KeyValue::new("demo.fault", fault)]);
    }

    pub(crate) fn record_starting(
        &self,
        address: &str,
        port: u16,
        movie_provider: &'static str,
        default_fault: &'static str,
        request_demo_faults_enabled: bool,
    ) {
        self.emit_event(serde_json::json!({
            "service_name": self.service_name,
            "event": "service.starting",
            "address": address,
            "port": port,
            "movie_provider": movie_provider,
            "default_fault": default_fault,
            "request_demo_faults_enabled": request_demo_faults_enabled
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

pub fn init(service_name: &'static str, otlp_endpoint: Option<&str>) -> Result<Telemetry> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer_provider = build_tracer_provider(service_name, otlp_endpoint);
    if let Some(provider) = &tracer_provider {
        global::set_tracer_provider(provider.clone());
    }

    let meter_provider = build_meter_provider(service_name, otlp_endpoint);
    global::set_meter_provider(meter_provider.clone());

    init_tracing_subscriber(service_name, tracer_provider.as_ref())?;
    let event_logger = EventLogger::new(EVENT_BUFFER_CAPACITY)
        .context("failed to initialize application event logger")?;

    Ok(Telemetry {
        service_name,
        metrics: build_metrics(meter_provider.meter(service_name)),
        tracer_provider,
        meter_provider,
        event_logger: Some(event_logger),
    })
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
            .with_resource(resource(service_name))
            .with_batch_exporter(exporter)
            .build(),
    )
}

fn build_meter_provider(
    service_name: &'static str,
    otlp_endpoint: Option<&str>,
) -> SdkMeterProvider {
    let Some(otlp_endpoint) = otlp_endpoint else {
        return SdkMeterProvider::builder()
            .with_resource(resource(service_name))
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
                .with_resource(resource(service_name))
                .build();
        }
    };

    SdkMeterProvider::builder()
        .with_resource(resource(service_name))
        .with_periodic_exporter(exporter)
        .build()
}

fn signal_endpoint(base_endpoint: &str, signal_path: &str) -> String {
    format!("{}/{signal_path}", base_endpoint.trim_end_matches('/'))
}

fn resource(service_name: &'static str) -> Resource {
    Resource::builder()
        .with_service_name(service_name)
        .with_attribute(KeyValue::new("demo.name", "multi-service-observability"))
        .build()
}

fn build_metrics(meter: Meter) -> TelemetryMetrics {
    TelemetryMetrics {
        http_requests: meter
            .u64_counter("movie_recommendation_service_http_requests_total")
            .with_description("Total inbound HTTP requests handled by route, status, and fault.")
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
        faults: meter
            .u64_counter("movie_recommendation_service_faults_total")
            .with_description("Total poison-pill fault activations.")
            .build(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_metric_labels_are_bounded_contract_values() {
        let labels = HttpMetricLabels {
            route: "/recommendations",
            status: 503,
            fault: "recommendation-error",
        };

        assert_eq!(labels.route, "/recommendations");
        assert_eq!(labels.status, 503);
        assert_eq!(labels.fault, "recommendation-error");
        assert_eq!(labels.attributes().len(), 3);
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
            (
                HttpEventKind::RecommendationsFaultError,
                "recommendations.fault_error",
            ),
            (
                HttpEventKind::RecommendationsFaultDelayCompleted,
                "recommendations.fault_delay_completed",
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
