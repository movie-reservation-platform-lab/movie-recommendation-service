use super::catalog::{seed_movies, selected_calibration};
use crate::domain::{
    movie::{Movie, MovieRecommendation},
    recommendation::{rank_movies, RatingCalibration},
};
use crate::services::movie::movie_service::AsyncMovieService;
use anyhow::Result;
use async_trait::async_trait;
use tracing::info_span;

pub struct FakeMovieService {
    calibration: fn() -> RatingCalibration,
}

impl Default for FakeMovieService {
    fn default() -> Self {
        Self {
            calibration: selected_calibration,
        }
    }
}

impl FakeMovieService {
    #[cfg(test)]
    pub(crate) fn with_calibration(calibration: fn() -> RatingCalibration) -> Self {
        Self { calibration }
    }
}

#[async_trait]
impl AsyncMovieService for FakeMovieService {
    async fn get_movies(&self, count: usize) -> Result<Vec<Movie>> {
        Ok(seed_movies().into_iter().take(count).collect())
    }

    async fn get_recommendations(
        &self,
        count: usize,
        preference: Option<String>,
    ) -> Result<Vec<MovieRecommendation>> {
        let span = info_span!("recommendations.rank",
            preference.present = preference.as_deref().is_some_and(|p| !p.trim().is_empty()),
            requested.count = count,
            otel.status_code = tracing::field::Empty,
            error.type = tracing::field::Empty,
        );
        let _entered = span.enter();
        let result = rank_movies(
            seed_movies(),
            count,
            preference.as_deref(),
            (self.calibration)(),
        );
        if let Err(error) = &result {
            span.record("otel.status_code", "ERROR");
            span.record("error.type", "non_finite_score");
            tracing::error!(event = "recommendations.ranking_failed", error.type = "non_finite_score", error = %error, "recommendation ranking failed");
        }
        result.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_service_contains_sibling_demo_seed_titles() {
        let service = FakeMovieService::with_calibration(RatingCalibration::default);

        let movies = service.get_movies(20).await.unwrap();
        let titles = movies
            .iter()
            .map(|movie| movie.title.as_str())
            .collect::<Vec<_>>();

        assert_eq!(movies.len(), 10);
        assert!(titles.contains(&"The Shawshank Redemption"));
        assert!(titles.contains(&"The Matrix"));
        assert!(titles.contains(&"The Type-Safe Matinee"));
        assert!(titles.contains(&"Fargate at Midnight"));
        assert!(titles.contains(&"The Last Deployment"));
        assert!(titles.contains(&"Casablanca"));
    }

    #[tokio::test]
    async fn recommendations_include_reservation_movie_ids() {
        let service = FakeMovieService::with_calibration(RatingCalibration::default);

        let recommendations = service
            .get_recommendations(3, Some("sci-fi".into()))
            .await
            .unwrap();

        assert_eq!(recommendations.len(), 3);
        assert!(recommendations
            .iter()
            .any(|recommendation| recommendation.title == "The Matrix"));
        assert!(recommendations[0].reason.contains("sci-fi"));
    }

    #[tokio::test]
    async fn recommendations_rank_by_preference_heuristic() {
        let service = FakeMovieService::with_calibration(RatingCalibration::default);

        let recommendations = service
            .get_recommendations(2, Some("platform deployment demo".into()))
            .await
            .unwrap();

        let titles = recommendations
            .iter()
            .map(|recommendation| recommendation.title.as_str())
            .collect::<Vec<_>>();

        assert!(titles.contains(&"Fargate at Midnight"));
        assert!(
            titles.contains(&"The Last Deployment") || titles.contains(&"The Type-Safe Matinee")
        );
    }
}
