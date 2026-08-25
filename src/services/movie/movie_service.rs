use crate::domain::movie::{Movie, MovieRecommendation};
use async_trait::async_trait;

#[async_trait]
pub trait AsyncMovieService: Send + Sync {
    async fn get_movies(&self, count: usize) -> anyhow::Result<Vec<Movie>>;

    async fn get_recommendations(
        &self,
        count: usize,
        preference: Option<String>,
    ) -> anyhow::Result<Vec<MovieRecommendation>>;
}
