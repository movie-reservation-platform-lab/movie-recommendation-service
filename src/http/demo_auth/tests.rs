use super::*;
use crate::{
    audit::sink::{write_event, AuditSink},
    demo_fault::FaultMode,
    services::movie::dummy::FakeMovieService,
    telemetry::Telemetry,
    SERVICE_NAME,
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderValue, Request},
};
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::{InMemorySpanExporter, Sampler, SdkTracerProvider};
use serde_json::{json, Value};
use std::{
    io,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;
use tracing_subscriber::prelude::*;

// These router tests share tracing's static login callsites. Registering their
// first use with no subscriber while another test installs a scoped subscriber
// races the interest cache. Isolate test dispatchers, not production requests;
// the concurrent-request test still exercises eight simultaneous handlers.
static TELEMETRY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturedLogs {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct MemorySink(Mutex<Vec<Vec<u8>>>);

impl AuditSink for MemorySink {
    fn write(&self, event: &AuthenticationEvent) -> io::Result<()> {
        let mut line = Vec::new();
        write_event(&mut line, event)?;
        self.0.lock().unwrap().push(line);
        Ok(())
    }
}

impl MemorySink {
    fn events(&self) -> Vec<Value> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|line| {
                assert_eq!(line.iter().filter(|byte| **byte == b'\n').count(), 1);
                assert_eq!(line.last(), Some(&b'\n'));
                let envelope: Value = serde_json::from_slice(line).unwrap();
                assert_eq!(envelope.as_object().unwrap().len(), 1);
                let event = envelope["audit"].clone();
                validate_event(&event);
                event
            })
            .collect()
    }
}

fn validate_event(event: &Value) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/platform-audit-event-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(event),
        "{}",
        validator
            .iter_errors(event)
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    );
}

fn app(sink: Arc<dyn AuditSink>, enabled: bool) -> Router {
    let credentials = DemoCredentials::from_values(
        Some(if enabled { "true" } else { "false" }),
        Some("test-user"),
        Some("fixture-password"),
    )
    .unwrap();
    super::router(
        credentials,
        EventIdentity::from_values(Some("demo-1+abc"), Some("test")).unwrap(),
        AuditEmitter::new(sink),
    )
}

fn request(body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/demo/auth/login")
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap()
}

#[tokio::test]
async fn correct_wrong_missing_and_malformed_credentials_are_audited_without_secrets() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    let cases = [
        (json!({"username":"test-user", "password":"fixture-password"}).to_string(), 200, "AUTHENTICATED"),
        (json!({"username":"unverified-username", "password":"wrong-secret-marker"}).to_string(), 401, "INVALID_CREDENTIALS"),
        ("{}".into(), 401, "MISSING_CREDENTIALS"),
        (json!({"username":"test-user"}).to_string(), 401, "MISSING_CREDENTIALS"),
        (json!({"username":"", "password":""}).to_string(), 401, "MISSING_CREDENTIALS"),
        ("{invalid secret-marker".into(), 400, "MALFORMED_CREDENTIALS"),
        (json!({"username":3, "password":false}).to_string(), 400, "MALFORMED_CREDENTIALS"),
        (json!({"username":"test-user", "password":"fixture-password", "unexpected":"secret-marker"}).to_string(), 400, "MALFORMED_CREDENTIALS"),
        ("[]".into(), 400, "MALFORMED_CREDENTIALS"),
        (json!({"username":"x".repeat(257), "password":"fixture-password"}).to_string(), 400, "MALFORMED_CREDENTIALS"),
        (json!({"username":"test-user", "password":"x".repeat(1025)}).to_string(), 400, "MALFORMED_CREDENTIALS"),
        ("x".repeat(17 * 1024), 400, "MALFORMED_CREDENTIALS"),
    ];
    for (payload, status, reason) in cases {
        let sink = Arc::new(MemorySink::default());
        let response = app(sink.clone(), true)
            .oneshot(request(payload))
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let response_body = body(response).await;
        assert_eq!(response_body["authenticated"], status == 200);
        assert_eq!(
            response_body["message"],
            if status == 200 {
                "Demo credentials accepted"
            } else {
                "Invalid credentials"
            }
        );
        assert!(
            response_body.get("trace_id").is_none(),
            "no OTel layer means no real trace"
        );
        let events = sink.events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event["status_detail"], reason);
        assert_eq!(event["metadata"]["uid"], response_body["audit_event_id"]);
        assert_eq!(
            event["unmapped"]["platform"]["request_id"],
            response_body["request_id"]
        );
        assert_eq!(event["service"]["name"], SERVICE_NAME);
        assert_eq!(event["service"]["version"], "demo-1+abc");
        assert_eq!(event["unmapped"]["platform"]["environment"], "test");
        let rendered = event.to_string();
        for secret in [
            "unverified-username",
            "fixture-password",
            "secret-marker",
            "test-user",
        ] {
            assert!(!rendered.contains(secret));
        }
    }
}

#[tokio::test]
async fn disabled_login_does_not_emit_and_existing_api_is_unchanged() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    let sink = Arc::new(MemorySink::default());
    let combined = crate::http::build_app(
        Arc::new(FakeMovieService),
        Arc::new(Telemetry::noop(SERVICE_NAME)),
        FaultMode::None,
        true,
    )
    .merge(app(sink.clone(), false));
    let response = combined.clone().oneshot(request("{}")).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    for path in ["/health", "/ready", "/movies", "/recommendations"] {
        let response = combined
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn sink_failure_never_reports_authentication_success() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    struct BrokenSink;
    impl AuditSink for BrokenSink {
        fn write(&self, _: &AuthenticationEvent) -> io::Result<()> {
            Err(io::Error::other("secret-writer-detail"))
        }
    }
    for payload in [
        "{}",
        "{\"username\":\"test-user\",\"password\":\"fixture-password\"}",
    ] {
        let response = app(Arc::new(BrokenSink), true)
            .oneshot(request(payload))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let response_body = body(response).await;
        assert_eq!(response_body["authenticated"], false);
        assert_eq!(response_body["message"], "Audit logging unavailable");
        assert!(!response_body.to_string().contains("secret-writer-detail"));
    }
}

#[tokio::test]
async fn request_native_context_is_bounded_and_not_treated_as_identity() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    let sink = Arc::new(MemorySink::default());
    let mut incoming = request("{}");
    incoming
        .headers_mut()
        .insert("x-correlation-id", HeaderValue::from_static("action-1"));
    incoming
        .headers_mut()
        .insert("x-request-id", HeaderValue::from_static("request-1"));
    incoming.headers_mut().insert(
        "x-amzn-trace-id",
        HeaderValue::from_static("Root=1-6a9dd271-0123456789abcdef01234567;Self=native"),
    );
    incoming
        .headers_mut()
        .insert("x-amz-cf-id", HeaderValue::from_static("cf-request+=="));
    let response = app(sink.clone(), true).oneshot(incoming).await.unwrap();
    assert_eq!(response.headers()["x-correlation-id"], "action-1");
    assert_eq!(response.headers()["x-request-id"], "request-1");
    let event = &sink.events()[0];
    assert_eq!(event["metadata"]["correlation_uid"], "action-1");
    assert_eq!(
        event["unmapped"]["platform"]["aws_cloudfront_request_id"],
        "cf-request+=="
    );
    assert!(event["unmapped"]["platform"]["aws_alb_trace_id"]
        .as_str()
        .unwrap()
        .starts_with("Root=1-"));
    assert_eq!(event["user"]["name"], "unknown");

    for value in ["x".repeat(513), "tab\tinjection".into()] {
        let mut incoming = request("{}");
        incoming
            .headers_mut()
            .insert("x-amzn-trace-id", HeaderValue::from_str(&value).unwrap());
        incoming
            .headers_mut()
            .insert("x-amz-cf-id", HeaderValue::from_str(&value).unwrap());
        incoming
            .headers_mut()
            .insert("x-request-id", HeaderValue::from_static("invalid+request"));
        incoming.headers_mut().insert(
            "x-correlation-id",
            HeaderValue::from_static("invalid correlation"),
        );
        app(sink.clone(), true).oneshot(incoming).await.unwrap();
    }
    for event in sink.events().iter().skip(1) {
        let platform = &event["unmapped"]["platform"];
        assert!(platform.get("aws_alb_trace_id").is_none());
        assert!(platform.get("aws_cloudfront_request_id").is_none());
        assert!(safe_id(platform["request_id"].as_str().unwrap()));
        assert!(safe_id(
            event["metadata"]["correlation_uid"].as_str().unwrap()
        ));
    }
}

#[tokio::test]
async fn exported_request_span_and_audit_share_actual_context() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let logs = CapturedLogs::default();
    let writer = logs.clone();
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .flatten_event(true)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(move || writer.clone()),
        )
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE_NAME)));
    // These Tokio tests use the current-thread runtime: keep the dispatcher
    // installed throughout the request instead of registering it per future poll.
    let _dispatcher = tracing::subscriber::set_default(subscriber);
    let sink = Arc::new(MemorySink::default());
    let mut incoming =
        request("{\"username\":\"unverified-username\",\"password\":\"secret-marker\"}");
    incoming.headers_mut().insert(
        "traceparent",
        HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
    );
    incoming
        .headers_mut()
        .insert("x-request-id", HeaderValue::from_static("request-exported"));
    incoming.headers_mut().insert(
        "x-amzn-trace-id",
        HeaderValue::from_static("Root=1-6a9dd271-0123456789abcdef01234567"),
    );
    let response = app(sink.clone(), true).oneshot(incoming).await.unwrap();
    let response_body = body(response).await;
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    let span = spans
        .iter()
        .find(|span| span.name == "http.request")
        .unwrap();
    let event = &sink.events()[0];
    let platform = &event["unmapped"]["platform"];
    assert_eq!(platform["trace_id"], "4bf92f3577b34da6a3ce929d0e0e4736");
    assert_eq!(
        platform["trace_id"],
        span.span_context.trace_id().to_string()
    );
    assert_eq!(platform["span_id"], span.span_context.span_id().to_string());
    assert_eq!(response_body["trace_id"], platform["trace_id"]);
    assert_eq!(span.parent_span_id.to_string(), "00f067aa0ba902b7");
    for (key, value) in [
        ("audit.event_id", event["metadata"]["uid"].as_str().unwrap()),
        ("app.request_id", "request-exported"),
        (
            "aws.alb.trace_id",
            platform["aws_alb_trace_id"].as_str().unwrap(),
        ),
    ] {
        assert!(
            span.attributes
                .iter()
                .any(|attribute| attribute.key.as_str() == key
                    && attribute.value.to_string() == value),
            "missing span attribute {key}"
        );
    }
    assert_eq!(span.span_kind, opentelemetry::trace::SpanKind::Server);
    let log_text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(!log_text.contains("secret-marker"));
    assert!(!log_text.contains("unverified-username"));
    let log: Value = log_text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|log| log["event"] == "audit.authentication")
        .unwrap();
    assert_eq!(log["audit_event_id"], event["metadata"]["uid"]);
    assert_eq!(log["trace_id"], platform["trace_id"]);
    assert_eq!(log["span_id"], platform["span_id"]);
    assert_eq!(log["aws_alb_trace_id"], platform["aws_alb_trace_id"]);
}

#[tokio::test]
async fn unsampled_context_is_retained_in_audit_without_claiming_an_exported_span() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::AlwaysOn)))
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE_NAME)));
    let _dispatcher = tracing::subscriber::set_default(subscriber);
    let sink = Arc::new(MemorySink::default());
    let mut incoming = request("{}");
    incoming.headers_mut().insert(
        "traceparent",
        HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00"),
    );
    let response = app(sink.clone(), true).oneshot(incoming).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        sink.events()[0]["unmapped"]["platform"]["trace_id"],
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    provider.force_flush().unwrap();
    assert!(exporter.get_finished_spans().unwrap().is_empty());
}

#[tokio::test]
async fn concurrent_requests_do_not_share_context_or_event_ids() {
    let _subscriber_guard = TELEMETRY_TEST_LOCK.lock().await;
    let sink = Arc::new(MemorySink::default());
    let app = app(sink.clone(), true);
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..8 {
        let app = app.clone();
        tasks.spawn(async move {
            let mut incoming = request("{}");
            incoming.headers_mut().insert(
                "x-request-id",
                HeaderValue::from_str(&format!("request-{index}")).unwrap(),
            );
            incoming.headers_mut().insert(
                "x-correlation-id",
                HeaderValue::from_str(&format!("action-{index}")).unwrap(),
            );
            let response = app.oneshot(incoming).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            body(response).await
        });
    }
    let mut responses = Vec::new();
    while let Some(response) = tasks.join_next().await {
        responses.push(response.unwrap());
    }
    let events = sink.events();
    assert_eq!(events.len(), 8);
    let ids: std::collections::HashSet<_> = events
        .iter()
        .map(|event| event["metadata"]["uid"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 8);
    for index in 0..8 {
        let event = events
            .iter()
            .find(|event| event["unmapped"]["platform"]["request_id"] == format!("request-{index}"))
            .unwrap();
        assert_eq!(
            event["metadata"]["correlation_uid"],
            format!("action-{index}")
        );
        assert!(responses.iter().any(|response| response["audit_event_id"]
            == event["metadata"]["uid"]
            && response["request_id"] == format!("request-{index}")));
    }
}
