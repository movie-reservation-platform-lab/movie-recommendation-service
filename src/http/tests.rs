use super::*;
use crate::{
    domain::movie::{Movie, MovieRecommendation},
    services::movie::dummy::FakeMovieService,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use serde_json::Value;
use tower::ServiceExt;

const MAX_TEST_BODY_BYTES: usize = 1024 * 1024;

fn test_app() -> Router {
    test_app_with(Arc::new(FakeMovieService), FaultMode::None)
}

fn test_app_with(movie_service: Arc<dyn AsyncMovieService>, default_fault: FaultMode) -> Router {
    build_app(
        movie_service,
        Arc::new(Telemetry::noop(SERVICE_NAME)),
        default_fault,
        true,
    )
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

async fn response_json(response: Response) -> Value {
    let body = to_bytes(response.into_body(), MAX_TEST_BODY_BYTES)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn health_returns_stable_contract_and_context_headers() {
    let response = test_app().oneshot(get("/health")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-correlation-id"));
    assert!(response.headers().contains_key("x-request-id"));

    let body = response_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service_name"], SERVICE_NAME);
}

#[tokio::test]
async fn readiness_returns_stable_contract() {
    let response = test_app().oneshot(get("/ready")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["status"], "ready");
    assert_eq!(body["service_name"], SERVICE_NAME);
}

#[tokio::test]
async fn movies_use_default_and_explicit_limits() {
    let default_response = test_app().oneshot(get("/movies")).await.unwrap();
    let explicit_response = test_app().oneshot(get("/movies?limit=3")).await.unwrap();

    assert_eq!(default_response.status(), StatusCode::OK);
    assert_eq!(explicit_response.status(), StatusCode::OK);

    let default_body = response_json(default_response).await;
    let explicit_body = response_json(explicit_response).await;
    assert_eq!(default_body.as_array().unwrap().len(), 10);
    assert_eq!(explicit_body.as_array().unwrap().len(), 3);
    assert_eq!(explicit_body[0]["title"], "The Shawshank Redemption");
    assert_eq!(
        explicit_body[0]["movie_reservation_movie_id"],
        "44444444-4444-4444-8444-444444444434"
    );
}

#[tokio::test]
async fn recommendations_use_default_and_explicit_limits() {
    let default_response = test_app().oneshot(get("/recommendations")).await.unwrap();
    let explicit_response = test_app()
        .oneshot(get("/recommendations?limit=2&preference=sci-fi"))
        .await
        .unwrap();

    assert_eq!(default_response.status(), StatusCode::OK);
    assert_eq!(explicit_response.status(), StatusCode::OK);

    let default_body = response_json(default_response).await;
    let explicit_body = response_json(explicit_response).await;
    assert_eq!(
        default_body["recommendations"].as_array().unwrap().len(),
        DEFAULT_RECOMMENDATION_LIMIT
    );
    let recommendations = explicit_body["recommendations"].as_array().unwrap();
    assert_eq!(recommendations.len(), 2);
    assert!(recommendations
        .iter()
        .all(|recommendation| recommendation["movie_reservation_movie_id"].is_string()));
    assert!(recommendations[0]["reason"]
        .as_str()
        .unwrap()
        .contains("sci-fi"));
}

#[test]
fn limits_are_defaulted_and_clamped() {
    let cases = [
        (None, DEFAULT_MOVIE_LIMIT, DEFAULT_MOVIE_LIMIT),
        (Some(0), DEFAULT_MOVIE_LIMIT, 0),
        (Some(MAX_LIMIT), DEFAULT_MOVIE_LIMIT, MAX_LIMIT),
        (Some(MAX_LIMIT + 1), DEFAULT_MOVIE_LIMIT, MAX_LIMIT),
    ];

    for (limit, default, expected) in cases {
        assert_eq!(bounded_limit(limit, default), expected);
    }
}

#[tokio::test]
async fn malformed_query_returns_safe_json_error() {
    let response = test_app()
        .oneshot(get("/movies?limit=not-a-number"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "invalid_query");
    assert_eq!(body["error"]["fault"], "none");
}

#[test]
fn preference_boundary_is_explicit() {
    assert_eq!(validated_preference(None).unwrap(), None);
    assert_eq!(validated_preference(Some("   ".into())).unwrap(), None);
    assert_eq!(
        validated_preference(Some(" ".repeat(MAX_PREFERENCE_BYTES + 1))).unwrap(),
        None
    );
    assert_eq!(
        validated_preference(Some(" sci-fi ".into())).unwrap(),
        Some("sci-fi".into())
    );
    assert!(validated_preference(Some("a".repeat(MAX_PREFERENCE_BYTES))).is_ok());
    assert!(validated_preference(Some("a".repeat(MAX_PREFERENCE_BYTES + 1))).is_err());
}

#[tokio::test]
async fn oversized_preference_returns_safe_json_error() {
    let preference = "a".repeat(MAX_PREFERENCE_BYTES + 1);
    let response = test_app()
        .oneshot(get(&format!("/recommendations?preference={preference}")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "invalid_preference");
    assert_eq!(
        body["error"]["message"],
        "Preference must be at most 128 bytes"
    );
}

#[tokio::test]
async fn recommendation_error_fault_returns_exact_503_contract() {
    let request = Request::builder()
        .uri("/recommendations")
        .header("x-demo-fault", "recommendation-error")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "recommendation_unavailable");
    assert_eq!(body["error"]["fault"], "recommendation-error");
}

#[tokio::test(start_paused = true)]
async fn slow_fault_uses_tokio_time_then_succeeds() {
    let started_at = Instant::now();
    let request = Request::builder()
        .uri("/recommendations?limit=1")
        .header("x-demo-fault", "slow-recommendation")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(started_at.elapsed(), SLOW_RECOMMENDATION_DELAY);
    let body = response_json(response).await;
    assert_eq!(body["recommendations"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn configured_fault_is_fallback_and_header_takes_precedence() {
    let configured_app = test_app_with(Arc::new(FakeMovieService), FaultMode::RecommendationError);
    let fallback_response = configured_app
        .clone()
        .oneshot(get("/recommendations"))
        .await
        .unwrap();
    let override_request = Request::builder()
        .uri("/recommendations")
        .header("x-demo-fault", "none")
        .body(Body::empty())
        .unwrap();
    let override_response = configured_app.oneshot(override_request).await.unwrap();

    assert_eq!(fallback_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(override_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn request_fault_header_is_ignored_when_gate_is_disabled() {
    let app = build_app(
        Arc::new(FakeMovieService),
        Arc::new(Telemetry::noop(SERVICE_NAME)),
        FaultMode::None,
        false,
    );
    let request = Request::builder()
        .uri("/recommendations?limit=1")
        .header("x-demo-fault", "recommendation-error")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_or_oversized_header_fault_is_safe_none() {
    let oversized = "a".repeat(MAX_FAULT_HEADER_BYTES + 1);
    for fault in ["unknown".to_owned(), oversized] {
        let request = Request::builder()
            .uri("/recommendations?limit=1")
            .header("x-demo-fault", fault)
            .body(Body::empty())
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn faults_do_not_affect_health_readiness_or_movies() {
    for uri in ["/health", "/ready", "/movies?limit=1"] {
        let request = Request::builder()
            .uri(uri)
            .header("x-demo-fault", "recommendation-error")
            .body(Body::empty())
            .unwrap();
        let response = test_app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "uri: {uri}");
    }
}

#[tokio::test]
async fn bounded_context_headers_are_echoed() {
    let request = Request::builder()
        .uri("/health")
        .header("x-correlation-id", "correlation-123")
        .header("x-request-id", "request-456")
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();

    assert_eq!(response.headers()["x-correlation-id"], "correlation-123");
    assert_eq!(response.headers()["x-request-id"], "request-456");
}

#[tokio::test]
async fn oversized_context_header_is_replaced() {
    let oversized = "a".repeat(context::MAX_CONTEXT_ID_BYTES + 1);
    let request = Request::builder()
        .uri("/health")
        .header("x-correlation-id", &oversized)
        .body(Body::empty())
        .unwrap();
    let response = test_app().oneshot(request).await.unwrap();

    let correlation_id = response.headers()["x-correlation-id"].to_str().unwrap();
    assert_ne!(correlation_id, oversized);
    assert!(correlation_id.len() <= context::MAX_CONTEXT_ID_BYTES);
}

#[tokio::test]
async fn movie_service_failure_returns_safe_500() {
    let response = test_app_with(Arc::new(FailingMovieService), FaultMode::None)
        .oneshot(get("/recommendations"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "internal_error");
    assert_eq!(body["error"]["message"], "Recommendation service failed");
    assert_eq!(body["error"]["fault"], "none");
    assert!(!body.to_string().contains("provider-secret"));
}

struct FailingMovieService;

#[async_trait]
impl AsyncMovieService for FailingMovieService {
    async fn get_movies(&self, _count: usize) -> anyhow::Result<Vec<Movie>> {
        anyhow::bail!("provider-secret diagnostic")
    }

    async fn get_recommendations(
        &self,
        _count: usize,
        _preference: Option<String>,
    ) -> anyhow::Result<Vec<MovieRecommendation>> {
        anyhow::bail!("provider-secret diagnostic")
    }
}
