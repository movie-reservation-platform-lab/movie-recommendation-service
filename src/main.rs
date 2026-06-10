mod di;
mod domain;
mod services;
mod telemetry;

use crate::di::movie_service::create_movie_service;
use crate::domain::movie::RecommendationResponse;
use crate::services::movie::movie_service::AsyncMovieService;
use crate::telemetry::Telemetry;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use opentelemetry::{global, propagation::Extractor, Context};
use serde::{Deserialize, Serialize};
use std::{
    env,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

const SERVICE_NAME: &str = "axum-tools-random-api";
const DEFAULT_PORT: u16 = 8082;
const DEFAULT_MOVIE_LIMIT: usize = 10;
const DEFAULT_RECOMMENDATION_LIMIT: usize = 5;
const MAX_LIMIT: usize = 20;
const SLOW_RECOMMENDATION_DELAY: Duration = Duration::from_secs(2);

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[tokio::main]
async fn main() {
    let telemetry = Arc::new(telemetry::init(SERVICE_NAME));
    let app = build_app(AppState {
        movie_service: create_movie_service(),
        telemetry: telemetry.clone(),
    });

    let port = configured_port();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!(
        "{}",
        serde_json::json!({
            "service_name": SERVICE_NAME,
            "event": "service.starting",
            "address": addr.to_string(),
            "port": port
        })
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    telemetry.shutdown();
}

#[derive(Clone)]
struct AppState {
    movie_service: Arc<dyn AsyncMovieService>,
    telemetry: Arc<Telemetry>,
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

#[derive(Clone, Debug)]
struct RequestContext {
    trace_id: String,
    correlation_id: String,
    request_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaultMode {
    None,
    SlowRecommendation,
    RecommendationError,
}

impl FaultMode {
    fn from_value(value: &str) -> Self {
        match value.trim() {
            value if value.eq_ignore_ascii_case("slow-recommendation") => Self::SlowRecommendation,
            value if value.eq_ignore_ascii_case("recommendation-error") => {
                Self::RecommendationError
            }
            _ => Self::None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SlowRecommendation => "slow-recommendation",
            Self::RecommendationError => "recommendation-error",
        }
    }
}

fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/movies", get(get_movies))
        .route("/recommendations", get(get_recommendations))
        .with_state(state)
}

async fn health(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let span = request_span("/health", FaultMode::None, &headers);
    async move {
        let started_at = Instant::now();
        let context = RequestContext::from_headers(&headers);
        let status = StatusCode::OK;
        let response = json_response(
            status,
            &context,
            HealthResponse {
                status: "ok",
                service_name: SERVICE_NAME,
            },
        );

        log_http_event(
            &state.telemetry,
            "health.completed",
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

async fn get_movies(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
    headers: HeaderMap,
) -> Response {
    let span = request_span("/movies", FaultMode::None, &headers);
    async move {
        let started_at = Instant::now();
        let context = RequestContext::from_headers(&headers);
        let limit = bounded_limit(query.limit, DEFAULT_MOVIE_LIMIT);

        match state.movie_service.get_movies(limit).await {
            Ok(movies) => {
                let status = StatusCode::OK;
                let response = json_response(status, &context, movies);
                log_http_event(
                    &state.telemetry,
                    "movies.completed",
                    &context,
                    FaultMode::None,
                    "/movies",
                    status,
                    started_at.elapsed(),
                );
                response
            }
            Err(_) => {
                internal_error_response(&state.telemetry, &context, "/movies", started_at.elapsed())
            }
        }
    }
    .instrument(span)
    .await
}

async fn get_recommendations(
    State(state): State<AppState>,
    Query(query): Query<RecommendationQuery>,
    headers: HeaderMap,
) -> Response {
    let fault = selected_fault(&headers, env::var("DEMO_FAULT_MODE").ok());
    let span = request_span("/recommendations", fault, &headers);
    async move {
        let started_at = Instant::now();
        let context = RequestContext::from_headers(&headers);

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
            log_http_event(
                &state.telemetry,
                "recommendations.fault_error",
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
        let preference_present = query
            .preference
            .as_deref()
            .map(str::trim)
            .is_some_and(|preference| !preference.is_empty());

        match state
            .movie_service
            .get_recommendations(limit, query.preference)
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
                    FaultMode::SlowRecommendation => "recommendations.fault_delay_completed",
                    _ => "recommendations.completed",
                };
                log_http_event(
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
                "/recommendations",
                started_at.elapsed(),
            ),
        }
    }
    .instrument(span)
    .await
}

impl RequestContext {
    fn from_headers(headers: &HeaderMap) -> Self {
        let trace_id = header_value(headers, "traceparent")
            .and_then(|traceparent| trace_id_from_traceparent(&traceparent))
            .unwrap_or_else(|| generated_id("trace"));

        let correlation_id = header_value(headers, "x-correlation-id")
            .unwrap_or_else(|| generated_id("correlation"));
        let request_id =
            header_value(headers, "x-request-id").unwrap_or_else(|| generated_id("request"));

        Self {
            trace_id,
            correlation_id,
            request_id,
        }
    }
}

fn configured_port() -> u16 {
    match env::var("PORT") {
        Ok(port) => port
            .parse::<u16>()
            .unwrap_or_else(|_| panic!("PORT must be a valid u16, got {port}")),
        Err(_) => DEFAULT_PORT,
    }
}

fn bounded_limit(limit: Option<usize>, default: usize) -> usize {
    limit.unwrap_or(default).min(MAX_LIMIT)
}

fn selected_fault(headers: &HeaderMap, env_fault: Option<String>) -> FaultMode {
    if let Some(header_fault) = header_value(headers, "x-demo-fault") {
        return FaultMode::from_value(&header_fault);
    }

    env_fault
        .as_deref()
        .map(FaultMode::from_value)
        .unwrap_or(FaultMode::None)
}

fn request_span(route: &'static str, fault: FaultMode, headers: &HeaderMap) -> tracing::Span {
    let span = tracing::info_span!(
        "http.request",
        service.name = SERVICE_NAME,
        http.method = "GET",
        http.route = route,
        demo.fault = fault.as_str()
    );
    let _ = span.set_parent(extract_otel_context(headers));
    span
}

fn extract_otel_context(headers: &HeaderMap) -> Context {
    global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderMapExtractor { headers })
    })
}

struct HeaderMapExtractor<'a> {
    headers: &'a HeaderMap,
}

impl Extractor for HeaderMapExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(|name| name.as_str()).collect()
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn trace_id_from_traceparent(traceparent: &str) -> Option<String> {
    let mut parts = traceparent.split('-');
    let version = parts.next()?;
    let trace_id = parts.next()?;

    if version.len() == 2
        && trace_id.len() == 32
        && trace_id
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        && trace_id != "00000000000000000000000000000000"
    {
        Some(trace_id.to_ascii_lowercase())
    } else {
        None
    }
}

fn generated_id(prefix: &str) -> String {
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    format!("{prefix}-{millis}-{sequence}")
}

fn json_response<T>(status: StatusCode, context: &RequestContext, body: T) -> Response
where
    T: Serialize,
{
    (status, response_headers(context), Json(body)).into_response()
}

fn internal_error_response(
    telemetry: &Telemetry,
    context: &RequestContext,
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
    log_http_event(
        telemetry,
        "request.failed",
        context,
        FaultMode::None,
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

fn log_http_event(
    telemetry: &Telemetry,
    event: &'static str,
    context: &RequestContext,
    fault: FaultMode,
    http_route: &'static str,
    http_status: StatusCode,
    duration: Duration,
) {
    let duration_ms = duration_ms(duration);
    telemetry.record_http_request(
        http_route,
        http_status.as_u16(),
        fault.as_str(),
        duration_ms,
    );

    println!(
        "{}",
        serde_json::json!({
            "service_name": SERVICE_NAME,
            "event": event,
            "trace_id": context.trace_id,
            "correlation_id": context.correlation_id,
            "request_id": context.request_id,
            "fault": fault.as_str(),
            "http_route": http_route,
            "http_status": http_status.as_u16(),
            "duration_ms": duration_ms
        })
    );
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::movie::dummy::FakeMovieService;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::Value;
    use tower::ServiceExt;

    fn test_app() -> Router {
        build_app(AppState {
            movie_service: Arc::new(FakeMovieService {}),
            telemetry: Arc::new(Telemetry::noop(SERVICE_NAME)),
        })
    }

    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-correlation-id"));
        assert!(response.headers().contains_key("x-request-id"));

        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service_name"], SERVICE_NAME);
    }

    #[tokio::test]
    async fn movies_return_seed_catalog_with_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/movies?limit=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_json(response).await;
        assert_eq!(body.as_array().unwrap().len(), 3);
        assert_eq!(body[0]["title"], "The Shawshank Redemption");
        assert_eq!(
            body[0]["movie_reservation_movie_id"],
            "44444444-4444-4444-8444-444444444434"
        );
    }

    #[tokio::test]
    async fn recommendations_return_requested_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/recommendations?limit=2&preference=sci-fi")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_json(response).await;
        let recommendations = body["recommendations"].as_array().unwrap();
        assert_eq!(recommendations.len(), 2);
        assert!(recommendations
            .iter()
            .all(|recommendation| recommendation["movie_reservation_movie_id"].is_string()));
        assert!(recommendations[0]["reason"]
            .as_str()
            .unwrap()
            .contains("sci-fi"));
    }

    #[tokio::test]
    async fn recommendation_error_fault_returns_503() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/recommendations")
                    .header("x-demo-fault", "recommendation-error")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "recommendation_unavailable");
        assert_eq!(body["error"]["fault"], "recommendation-error");
    }

    #[tokio::test]
    async fn slow_recommendation_fault_delays_then_succeeds() {
        let started_at = Instant::now();
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/recommendations?limit=1")
                    .header("x-demo-fault", "slow-recommendation")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(started_at.elapsed() >= SLOW_RECOMMENDATION_DELAY);

        let body = response_json(response).await;
        assert_eq!(body["recommendations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn header_fault_takes_precedence_over_env_fault() {
        let mut headers = HeaderMap::new();
        headers.insert("x-demo-fault", HeaderValue::from_static("none"));

        assert_eq!(
            selected_fault(&headers, Some("recommendation-error".into())),
            FaultMode::None
        );
    }

    #[test]
    fn env_fault_is_used_when_header_is_absent() {
        assert_eq!(
            selected_fault(&HeaderMap::new(), Some("recommendation-error".into())),
            FaultMode::RecommendationError
        );
    }
}
