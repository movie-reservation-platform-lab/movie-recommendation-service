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
    test_app_with(Arc::new(FakeMovieService::with_calibration(
        crate::domain::recommendation::RatingCalibration::default,
    )))
}

fn test_app_with(movie_service: Arc<dyn AsyncMovieService>) -> Router {
    build_app(movie_service, Arc::new(Telemetry::noop(SERVICE_NAME)))
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
    let response = test_app_with(Arc::new(FailingMovieService))
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

fn single_rating_calibration() -> crate::domain::recommendation::RatingCalibration {
    crate::domain::recommendation::RatingCalibration {
        minimum: 8.0,
        maximum: 8.0,
    }
}

#[tokio::test(start_paused = true)]
async fn request_metadata_cannot_select_or_bypass_ranking_outcomes() {
    use crate::domain::recommendation::RatingCalibration;
    for (calibration, expected) in [
        (
            RatingCalibration::default as fn() -> RatingCalibration,
            StatusCode::OK,
        ),
        (single_rating_calibration, StatusCode::INTERNAL_SERVER_ERROR),
    ] {
        let app = test_app_with(Arc::new(FakeMovieService::with_calibration(calibration)));
        for fault in [
            None,
            Some("none"),
            Some("slow-recommendation"),
            Some("recommendation-error"),
            Some("unknown"),
        ] {
            for limit in [0, 1, 5, 20, 100] {
                let mut request = Request::builder()
                    .uri(format!("/recommendations?limit={limit}&preference=sci-fi&fault=none&snapshot=0&seed=1"))
                    .header("x-correlation-id", "ranking-request")
                    .header("x-request-id", "ranking-attempt");
                if let Some(fault) = fault {
                    request = request.header("x-demo-fault", fault);
                }
                let started = Instant::now();
                let response = app
                    .clone()
                    .oneshot(request.body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), expected);
                assert_eq!(started.elapsed(), Duration::ZERO);
                assert_eq!(response.headers()["x-correlation-id"], "ranking-request");
                assert_eq!(response.headers()["x-request-id"], "ranking-attempt");
                let body = response_json(response).await;
                if expected == StatusCode::INTERNAL_SERVER_ERROR {
                    assert_eq!(
                        body,
                        serde_json::json!({"error": {"code": "internal_error", "message": "Recommendation service failed", "fault": "none"}})
                    );
                } else {
                    assert_eq!(
                        body["recommendations"].as_array().unwrap().len(),
                        limit.min(10)
                    );
                }
            }
        }
        for uri in ["/health", "/ready", "/movies"] {
            let response = app.clone().oneshot(get(uri)).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let response = app
            .oneshot(get("/recommendations?limit=invalid"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn ranking_failure_exports_correlated_error_spans() {
    use opentelemetry::trace::{Status, TracerProvider};
    use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
    use tracing_subscriber::prelude::*;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE_NAME)));
    let _dispatcher = tracing::subscriber::set_default(subscriber);
    let app = test_app_with(Arc::new(FakeMovieService::with_calibration(
        single_rating_calibration,
    )));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/recommendations")
                .header(
                    "traceparent",
                    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    let http = spans
        .iter()
        .find(|span| span.name == "http.request")
        .unwrap();
    let rank = spans
        .iter()
        .find(|span| span.name == "recommendations.rank")
        .unwrap();
    assert_eq!(rank.parent_span_id, http.span_context.span_id());
    for span in [http, rank] {
        assert_eq!(
            span.span_context.trace_id().to_string(),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert!(matches!(span.status, Status::Error { .. }));
    }
    assert!(rank
        .attributes
        .iter()
        .any(|attribute| attribute.key.as_str() == "error.type"
            && attribute.value.as_str() == "non_finite_score"));
}
