use crate::domain::movie::Movie;
use async_trait::async_trait;



pub trait MovieService: Send + Sync {
    fn get_random_movies(&self, count: usize) -> anyhow::Result<Vec<Movie>>;
}

#[async_trait]
pub trait AsyncMovieService: Send + Sync {
    async fn get_random_movies(&self, count: usize) -> anyhow::Result<Vec<Movie>>;
}
