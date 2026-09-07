use super::{context::RequestContext, json_response};
use crate::audit::{
    config::{DemoCredentials, EventIdentity},
    event::{safe_id, safe_native_id, AuditContext, AuthenticationEvent, Outcome},
    sink::AuditEmitter,
};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
    Json, Router,
};
use opentelemetry::trace::TraceContextExt;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

#[derive(Clone)]
struct DemoState {
    credentials: DemoCredentials,
    identity: EventIdentity,
    emitter: AuditEmitter,
}

pub(crate) fn router(
    credentials: Option<DemoCredentials>,
    identity: EventIdentity,
    emitter: AuditEmitter,
) -> Router {
    let Some(credentials) = credentials else {
        return Router::new();
    };
    Router::new()
        .route("/demo/auth/login", post(login))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(DemoState {
            credentials,
            identity,
            emitter,
        })
}

// Do not derive Debug: credentials must not appear in diagnostics.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    username: Option<String>,
    password: Option<String>,
}

#[derive(Serialize)]
struct LoginResponse {
    authenticated: bool,
    message: &'static str,
    request_id: String,
    audit_event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
}

async fn login(
    State(state): State<DemoState>,
    headers: HeaderMap,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Response {
    let (mut context, parent) = RequestContext::from_headers(&headers);
    // The existing public routes keep their context contract; audit IDs use a stricter allowlist.
    if !safe_id(&context.request_id) {
        context.request_id = Uuid::new_v4().to_string();
    }
    if !safe_id(&context.correlation_id) {
        context.correlation_id = Uuid::new_v4().to_string();
    }
    let alb_id = native_header(&headers, "x-amzn-trace-id");
    let cloudfront_id = native_header(&headers, "x-amz-cf-id");
    let event_id = Uuid::new_v4();
    let span = tracing::info_span!(
        "http.request",
        otel.kind = "server",
        service.name = crate::SERVICE_NAME,
        http.method = "POST",
        http.route = "/demo/auth/login",
        app.request_id = %context.request_id,
        app.correlation_id = %context.correlation_id,
        audit.event_id = %event_id,
        aws.alb.trace_id = alb_id.as_deref().unwrap_or(""),
        aws.cloudfront.request_id = cloudfront_id.as_deref().unwrap_or(""),
        http.status_code = tracing::field::Empty,
        audit.outcome = tracing::field::Empty,
    );
    let _ = span.set_parent(parent);

    async move {
        let outcome = credential_outcome(&state.credentials, payload);
        let active_context = tracing::Span::current().context();
        let active = active_context.span();
        let span_context = active.span_context();
        // Never use RequestContext's synthetic fallback trace ID as an exported span ID.
        let (trace_id, span_id) = if span_context.is_valid() {
            (
                Some(span_context.trace_id().to_string()),
                Some(span_context.span_id().to_string()),
            )
        } else {
            (None, None)
        };
        let event = AuthenticationEvent::new(
            outcome,
            &state.identity,
            AuditContext {
                request_id: context.request_id.clone(),
                correlation_id: context.correlation_id.clone(),
                trace_id: trace_id.clone(),
                span_id: span_id.clone(),
                aws_alb_trace_id: alb_id.clone(),
                aws_cloudfront_request_id: cloudfront_id.clone(),
            },
            event_id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
        );
        let written = match event {
            Ok(event) => state.emitter.emit(event).await.is_ok(),
            Err(_) => false,
        };
        let status = if !written {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            match outcome {
                Outcome::Authenticated => StatusCode::OK,
                Outcome::MalformedCredentials => StatusCode::BAD_REQUEST,
                Outcome::InvalidCredentials | Outcome::MissingCredentials => {
                    StatusCode::UNAUTHORIZED
                }
            }
        };
        let authenticated = written && outcome.authenticated();
        let current = tracing::Span::current();
        current.record("http.status_code", status.as_u16());
        current.record("audit.outcome", outcome.reason());
        current.add_event(
            "audit.authentication",
            vec![
                opentelemetry::KeyValue::new("audit.event_id", event_id.to_string()),
                opentelemetry::KeyValue::new("audit.stdout_written", written),
            ],
        );
        // Correlation only: never format payload, validator input, or sink errors.
        tracing::info!(
            event = "audit.authentication",
            service_name = crate::SERVICE_NAME,
            service_version = %state.identity.version,
            deployment_environment = %state.identity.environment,
            audit_event_id = %event_id,
            request_id = %context.request_id,
            correlation_id = %context.correlation_id,
            trace_id = trace_id.as_deref().unwrap_or(""),
            span_id = span_id.as_deref().unwrap_or(""),
            aws_alb_trace_id = alb_id.as_deref().unwrap_or(""),
            aws_cloudfront_request_id = cloudfront_id.as_deref().unwrap_or(""),
            outcome = outcome.reason(),
            stdout_written = written,
            http_status = status.as_u16(),
            "demo credential check completed"
        );
        let mut response = json_response(
            status,
            &context,
            LoginResponse {
                authenticated,
                message: if !written {
                    "Audit logging unavailable"
                } else if authenticated {
                    "Demo credentials accepted"
                } else {
                    "Invalid credentials"
                },
                request_id: context.request_id.clone(),
                audit_event_id: event_id.to_string(),
                trace_id,
            },
        );
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        response
    }
    .instrument(span)
    .await
}

fn credential_outcome(
    credentials: &DemoCredentials,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Outcome {
    let Ok(Json(request)) = payload else {
        return Outcome::MalformedCredentials;
    };
    let (Some(username), Some(password)) = (request.username, request.password) else {
        return Outcome::MissingCredentials;
    };
    if username.chars().count() > 256 || password.chars().count() > 1024 {
        return Outcome::MalformedCredentials;
    }
    if username.is_empty() || password.is_empty() {
        return Outcome::MissingCredentials;
    }
    if credentials.matches(&username, &password) {
        Outcome::Authenticated
    } else {
        Outcome::InvalidCredentials
    }
}

fn native_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| safe_native_id(value))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
