use opentelemetry::{
    global,
    metrics::{Counter, Histogram},
    KeyValue,
};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    metrics::SdkMeterProvider, propagation::TraceContextPropagator, trace::SdkTracerProvider,
    Resource,
};
use std::env;
use tracing::warn;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

#[derive(Clone)]
pub struct Telemetry {
    metrics: TelemetryMetrics,
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

#[derive(Clone)]
struct TelemetryMetrics {
    http_requests: Counter<u64>,
    http_request_duration: Histogram<f64>,
    recommendations: Counter<u64>,
    faults: Counter<u64>,
}

impl Telemetry {
    #[cfg(test)]
    pub fn noop(service_name: &'static str) -> Self {
        Self {
            metrics: build_metrics(service_name),
            tracer_provider: None,
            meter_provider: None,
        }
    }

    pub fn record_http_request(
        &self,
        route: &'static str,
        status: u16,
        fault: &'static str,
        duration_ms: u64,
    ) {
        let attributes = [
            KeyValue::new("http.route", route),
            KeyValue::new("http.status_code", i64::from(status)),
            KeyValue::new("demo.fault", fault),
        ];

        self.metrics.http_requests.add(1, &attributes);
        self.metrics
            .http_request_duration
            .record(duration_ms as f64, &attributes);
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

    pub fn shutdown(&self) {
        if let Some(meter_provider) = &self.meter_provider {
            if let Err(error) = meter_provider.shutdown() {
                warn!(error = %error, "failed to shutdown OpenTelemetry meter provider");
            }
        }

        if let Some(tracer_provider) = &self.tracer_provider {
            if let Err(error) = tracer_provider.shutdown() {
                warn!(error = %error, "failed to shutdown OpenTelemetry tracer provider");
            }
        }
    }
}

pub fn init(service_name: &'static str) -> Telemetry {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer_provider = build_tracer_provider(service_name);
    if let Some(provider) = &tracer_provider {
        global::set_tracer_provider(provider.clone());
    }

    let meter_provider = build_meter_provider(service_name);
    if let Some(provider) = &meter_provider {
        global::set_meter_provider(provider.clone());
    }

    init_tracing_subscriber(service_name, tracer_provider.as_ref());

    Telemetry {
        metrics: build_metrics(service_name),
        tracer_provider,
        meter_provider,
    }
}

fn init_tracing_subscriber(
    service_name: &'static str,
    tracer_provider: Option<&SdkTracerProvider>,
) {
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
        .init();
}

fn build_tracer_provider(service_name: &'static str) -> Option<SdkTracerProvider> {
    if env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_err() {
        return None;
    }

    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()
    {
        Ok(exporter) => exporter,
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "service_name": service_name,
                    "event": "otel.trace_exporter.disabled",
                    "error": error.to_string()
                })
            );
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

fn build_meter_provider(service_name: &'static str) -> Option<SdkMeterProvider> {
    if env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_err() {
        return None;
    }

    let exporter = match opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()
    {
        Ok(exporter) => exporter,
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "service_name": service_name,
                    "event": "otel.metric_exporter.disabled",
                    "error": error.to_string()
                })
            );
            return None;
        }
    };

    Some(
        SdkMeterProvider::builder()
            .with_resource(resource(service_name))
            .with_periodic_exporter(exporter)
            .build(),
    )
}

fn resource(service_name: &'static str) -> Resource {
    Resource::builder()
        .with_service_name(service_name)
        .with_attributes([
            KeyValue::new("service.environment", "local"),
            KeyValue::new("demo.name", "multi-service-observability"),
        ])
        .build()
}

fn build_metrics(service_name: &'static str) -> TelemetryMetrics {
    let meter = global::meter_provider().meter(service_name);

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
