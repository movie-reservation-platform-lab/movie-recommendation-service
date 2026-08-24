use axum::http::HeaderMap;
use opentelemetry::{
    propagation::{Extractor, TextMapPropagator},
    trace::TraceContextExt,
    Context,
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const MAX_CONTEXT_ID_BYTES: usize = 128;
const MAX_TRACE_CONTEXT_HEADER_BYTES: usize = 512;

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub(super) struct RequestContext {
    pub(super) trace_id: String,
    pub(super) correlation_id: String,
    pub(super) request_id: String,
}

impl RequestContext {
    pub(super) fn from_headers(headers: &HeaderMap) -> (Self, Context) {
        let parent_context = extract_otel_context(headers);
        let trace_id = {
            let span = parent_context.span();
            let span_context = span.span_context();
            if span_context.is_valid() {
                span_context.trace_id().to_string()
            } else {
                generated_trace_id()
            }
        };

        let correlation_id = bounded_header_value(headers, "x-correlation-id")
            .unwrap_or_else(|| generated_id("correlation"));
        let request_id = bounded_header_value(headers, "x-request-id")
            .unwrap_or_else(|| generated_id("request"));

        (
            Self {
                trace_id,
                correlation_id,
                request_id,
            },
            parent_context,
        )
    }
}

pub(super) fn bounded_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .filter(|value| value.as_bytes().len() <= MAX_CONTEXT_ID_BYTES)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_otel_context(headers: &HeaderMap) -> Context {
    TraceContextPropagator::new().extract(&HeaderMapExtractor { headers })
}

struct HeaderMapExtractor<'a> {
    headers: &'a HeaderMap,
}

impl Extractor for HeaderMapExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers
            .get(key)
            .filter(|value| value.as_bytes().len() <= MAX_TRACE_CONTEXT_HEADER_BYTES)
            .and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(|name| name.as_str()).collect()
    }
}

fn generated_trace_id() -> String {
    let sequence = u128::from(REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let time_component = nanos & u128::from(u64::MAX);
    let value = (time_component << u64::BITS) | sequence;
    format!("{value:032x}")
}

fn generated_id(prefix: &str) -> String {
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    format!("{prefix}-{millis}-{sequence}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const VALID_TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

    #[test]
    fn extracts_valid_w3c_trace_id() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        );

        let (context, parent) = RequestContext::from_headers(&headers);

        assert_eq!(context.trace_id, VALID_TRACE_ID);
        assert!(parent.span().span_context().is_remote());
    }

    #[test]
    fn rejects_malformed_and_zero_w3c_context() {
        let cases = [
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-zz",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra",
            "malformed",
        ];

        for traceparent in cases {
            let mut headers = HeaderMap::new();
            headers.insert("traceparent", HeaderValue::from_str(traceparent).unwrap());

            let (context, parent) = RequestContext::from_headers(&headers);

            assert_ne!(context.trace_id, VALID_TRACE_ID, "value: {traceparent}");
            assert_eq!(context.trace_id.len(), 32);
            assert!(!parent.span().span_context().is_valid());
        }
    }

    #[test]
    fn preserves_bounded_request_metadata() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_static("correlation-123"),
        );
        headers.insert("x-request-id", HeaderValue::from_static("request-456"));

        let (context, _) = RequestContext::from_headers(&headers);

        assert_eq!(context.correlation_id, "correlation-123");
        assert_eq!(context.request_id, "request-456");
    }

    #[test]
    fn replaces_empty_or_oversized_request_metadata() {
        let oversized = "a".repeat(MAX_CONTEXT_ID_BYTES + 1);
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_str(&oversized).unwrap(),
        );
        headers.insert("x-request-id", HeaderValue::from_static("   "));

        let (context, _) = RequestContext::from_headers(&headers);

        assert_ne!(context.correlation_id, oversized);
        assert!(context.correlation_id.len() <= MAX_CONTEXT_ID_BYTES);
        assert!(context.request_id.starts_with("request-"));
    }
}
