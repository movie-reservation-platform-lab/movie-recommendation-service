mod context;
pub(crate) mod demo_auth;

#[cfg(test)]
mod tests;

use crate::{
    demo_fault::FaultMode,
    domain::movie::RecommendationResponse,
    services::movie::movie_service::AsyncMovieService,
    telemetry::{HttpEventKind, HttpRequestEvent, Telemetry},
    SERVICE_NAME,
};
use axum::{
    extract::{rejection::QueryRejection, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use context::RequestContext;
use opentelemetry::{trace::TraceContextExt, Context};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::time::{sleep, Instant};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

const DEFAULT_MOVIE_LIMIT: usize = 10;
const DEFAULT_RECOMMENDATION_LIMIT: usize = 5;
const MAX_LIMIT: usize = 20;
const MAX_PREFERENCE_BYTES: usize = 128;
const MAX_FAULT_HEADER_BYTES: usize = 64;
const SLOW_RECOMMENDATION_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct AppState {
    movie_service: Arc<dyn AsyncMovieService>,
    telemetry: Arc<Telemetry>,
    default_fault: FaultMode,
    allow_request_demo_faults: bool,
}

#[derive(Debug, Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RecommendationQuery {
    limit: Option<usize>,
    preference: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service_name: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorResponse,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: &'static str,
    fault: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct ClientError {
    code: &'static str,
    message: &'static str,
}

pub(crate) fn build_app(
    movie_service: Arc<dyn AsyncMovieService>,
    telemetry: Arc<Telemetry>,
    default_fault: FaultMode,
    allow_request_demo_faults: bool,
) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/movies", get(get_movies))
        .route("/recommendations", get(get_recommendations))
        .with_state(AppState {
            movie_service,
            telemetry,
            default_fault,
            allow_request_demo_faults,
        })
}

async fn health(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (context, parent_context) = RequestContext::from_headers(&headers);
    let span = request_span("/health", FaultMode::None, parent_context);

    async move {
        let started_at = Instant::now();
        let status = StatusCode::OK;
        let response = json_response(
            status,
            &context,
            HealthResponse {
                status: "ok",
                service_name: SERVICE_NAME,
            },
        );

        record_http_event(
            &state.telemetry,
            HttpEventKind::HealthCompleted,
            &context,
            FaultMode::None,
            "/health",
            status,
            started_at.elapsed(),
        );

        response
    }
    .instrument(span)
    .await
}

async fn readiness(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (context, parent_context) = RequestContext::from_headers(&headers);
    let span = request_span("/ready", FaultMode::None, parent_context);

    async move {
        let started_at = Instant::now();
        let status = StatusCode::OK;
        let response = json_response(
            status,
            &context,
            HealthResponse {
                status: "ready",
                service_name: SERVICE_NAME,
            },
        );

        record_http_event(
            &state.telemetry,
            HttpEventKind::ReadinessCompleted,
            &context,
            FaultMode::None,
            "/ready",
            status,
            started_at.elapsed(),
        );

        response
    }
    .instrument(span)
    .await
}

async fn get_movies(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<LimitQuery>, QueryRejection>,
) -> Response {
    let (context, parent_context) = RequestContext::from_headers(&headers);
    let span = request_span("/movies", FaultMode::None, parent_context);

    async move {
        let started_at = Instant::now();
        let query = match query {
            Ok(Query(query)) => query,
            Err(_) => {
                return client_error_response(
                    &state.telemetry,
                    &context,
                    "/movies",
                    started_at.elapsed(),
                    ClientError {
                        code: "invalid_query",
                        message: "Query parameters are invalid",
                    },
                );
            }
        };
        let limit = bounded_limit(query.limit, DEFAULT_MOVIE_LIMIT);

        match state.movie_service.get_movies(limit).await {
            Ok(movies) => {
                let status = StatusCode::OK;
                let response = json_response(status, &context, movies);
                record_http_event(
                    &state.telemetry,
                    HttpEventKind::MoviesCompleted,
                    &context,
                    FaultMode::None,
                    "/movies",
                    status,
                    started_at.elapsed(),
                );
                response
            }
            Err(_) => internal_error_response(
                &state.telemetry,
                &context,
                FaultMode::None,
                "/movies",
                started_at.elapsed(),
            ),
        }
    }
    .instrument(span)
    .await
}

async fn get_recommendations(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<RecommendationQuery>, QueryRejection>,
) -> Response {
    let fault = selected_fault(
        &headers,
        state.default_fault,
        state.allow_request_demo_faults,
    );
    let (context, parent_context) = RequestContext::from_headers(&headers);
    let span = request_span("/recommendations", fault, parent_context);

    async move {
        let started_at = Instant::now();
        let query = match query {
            Ok(Query(query)) => query,
            Err(_) => {
                return client_error_response(
                    &state.telemetry,
                    &context,
                    "/recommendations",
                    started_at.elapsed(),
                    ClientError {
                        code: "invalid_query",
                        message: "Query parameters are invalid",
                    },
                );
            }
        };
        let preference = match validated_preference(query.preference) {
            Ok(preference) => preference,
            Err(error) => {
                return client_error_response(
                    &state.telemetry,
                    &context,
                    "/recommendations",
                    started_at.elapsed(),
                    error,
                );
            }
        };

        if fault == FaultMode::RecommendationError {
            let status = StatusCode::SERVICE_UNAVAILABLE;
            let response = json_response(
                status,
                &context,
                ErrorEnvelope {
                    error: ErrorResponse {
                        code: "recommendation_unavailable",
                        message: "Recommendation service unavailable for demo fault",
                        fault: fault.as_str(),
                    },
                },
            );
            state.telemetry.record_fault(fault.as_str());
            record_http_event(
                &state.telemetry,
                HttpEventKind::RecommendationsFaultError,
                &context,
                fault,
                "/recommendations",
                status,
                started_at.elapsed(),
            );
            return response;
        }

        if fault == FaultMode::SlowRecommendation {
            state.telemetry.record_fault(fault.as_str());
            sleep(SLOW_RECOMMENDATION_DELAY)
                .instrument(tracing::info_span!(
                    "recommendations.fault_delay",
                    demo.fault = fault.as_str(),
                    delay_ms = duration_ms(SLOW_RECOMMENDATION_DELAY)
                ))
                .await;
        }

        let limit = bounded_limit(query.limit, DEFAULT_RECOMMENDATION_LIMIT);
        let preference_present = preference.is_some();

        match state
            .movie_service
            .get_recommendations(limit, preference)
            .await
        {
            Ok(recommendations) => {
                let status = StatusCode::OK;
                state
                    .telemetry
                    .record_recommendations(recommendations.len(), preference_present);
                let response =
                    json_response(status, &context, RecommendationResponse { recommendations });
                let event = match fault {
                    FaultMode::SlowRecommendation => {
                        HttpEventKind::RecommendationsFaultDelayCompleted
                    }
                    _ => HttpEventKind::RecommendationsCompleted,
                };
                record_http_event(
                    &state.telemetry,
                    event,
                    &context,
                    fault,
                    "/recommendations",
                    status,
                    started_at.elapsed(),
                );
                response
            }
            Err(_) => internal_error_response(
                &state.telemetry,
                &context,
                fault,
                "/recommendations",
                started_at.elapsed(),
            ),
        }
    }
    .instrument(span)
    .await
}

fn bounded_limit(limit: Option<usize>, default: usize) -> usize {
    limit.unwrap_or(default).min(MAX_LIMIT)
}

fn validated_preference(preference: Option<String>) -> Result<Option<String>, ClientError> {
    let Some(preference) = preference else {
        return Ok(None);
    };

    let preference = preference.trim();
    if preference.len() > MAX_PREFERENCE_BYTES {
        return Err(ClientError {
            code: "invalid_preference",
            message: "Preference must be at most 128 bytes",
        });
    }

    if preference.is_empty() {
        Ok(None)
    } else {
        Ok(Some(preference.to_owned()))
    }
}

fn selected_fault(
    headers: &HeaderMap,
    default_fault: FaultMode,
    allow_request_demo_faults: bool,
) -> FaultMode {
    if !allow_request_demo_faults {
        return default_fault;
    }

    let Some(value) = headers.get("x-demo-fault") else {
        return default_fault;
    };

    if value.as_bytes().len() > MAX_FAULT_HEADER_BYTES {
        return FaultMode::None;
    }

    value
        .to_str()
        .ok()
        .map(FaultMode::from_value)
        .unwrap_or_default()
}

fn request_span(route: &'static str, fault: FaultMode, parent_context: Context) -> tracing::Span {
    let span = tracing::info_span!(
        "http.request",
        service.name = SERVICE_NAME,
        http.method = "GET",
        http.route = route,
        demo.fault = fault.as_str()
    );
    let _ = span.set_parent(parent_context);
    span
}

fn json_response<T>(status: StatusCode, context: &RequestContext, body: T) -> Response
where
    T: Serialize,
{
    (status, response_headers(context), Json(body)).into_response()
}

fn client_error_response(
    telemetry: &Telemetry,
    context: &RequestContext,
    route: &'static str,
    duration: Duration,
    error: ClientError,
) -> Response {
    let status = StatusCode::BAD_REQUEST;
    let response = json_response(
        status,
        context,
        ErrorEnvelope {
            error: ErrorResponse {
                code: error.code,
                message: error.message,
                fault: FaultMode::None.as_str(),
            },
        },
    );
    record_http_event(
        telemetry,
        HttpEventKind::RequestRejected,
        context,
        FaultMode::None,
        route,
        status,
        duration,
    );
    response
}

fn internal_error_response(
    telemetry: &Telemetry,
    context: &RequestContext,
    fault: FaultMode,
    route: &'static str,
    duration: Duration,
) -> Response {
    let status = StatusCode::INTERNAL_SERVER_ERROR;
    let response = json_response(
        status,
        context,
        ErrorEnvelope {
            error: ErrorResponse {
                code: "internal_error",
                message: "Recommendation service failed",
                fault: FaultMode::None.as_str(),
            },
        },
    );
    record_http_event(
        telemetry,
        HttpEventKind::RequestFailed,
        context,
        fault,
        route,
        status,
        duration,
    );
    response
}

fn response_headers(context: &RequestContext) -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_response_header(&mut headers, "x-correlation-id", &context.correlation_id);
    insert_response_header(&mut headers, "x-request-id", &context.request_id);
    headers
}

fn insert_response_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}

fn record_http_event(
    telemetry: &Telemetry,
    event: HttpEventKind,
    context: &RequestContext,
    fault: FaultMode,
    http_route: &'static str,
    http_status: StatusCode,
    duration: Duration,
) {
    let duration_ms = duration_ms(duration);
    let span_context = tracing::Span::current().context();
    let span = span_context.span();
    let trace_id = if span.span_context().is_valid() {
        span.span_context().trace_id().to_string()
    } else {
        context.trace_id.clone()
    };

    telemetry.record_http_event(HttpRequestEvent {
        kind: event,
        trace_id: &trace_id,
        correlation_id: &context.correlation_id,
        request_id: &context.request_id,
        fault: fault.as_str(),
        route: http_route,
        status: http_status.as_u16(),
        duration_ms,
    });
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}
