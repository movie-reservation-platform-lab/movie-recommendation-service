use crate::domain::movie::Movie;
use crate::services::movie::movie_service::AsyncMovieService;
use anyhow::{Error, Result};
use async_trait::async_trait;

pub struct FakeMovieService;

#[async_trait]
impl AsyncMovieService for FakeMovieService {
    async fn get_random_movies(&self, count: usize) -> Result<Vec<Movie>, Error> {
        Ok(vec![
            Movie {
                id: 1,
                title: "The Matrix".into(),
                overview: "Simulation awakening.".into(),
                release_date: Some("1999".to_string()),
                vote_average: 9.9,
            },
            Movie {
                id: 2,
                title: "Interstellar".into(),
                overview: "Wormhole explorers.".into(),
                release_date: Some("2024".to_string()),
                vote_average: 9.2,
            },
        ]
        .into_iter()
        .take(count)
        .collect())
    }
}
