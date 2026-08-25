use crate::config::MovieProvider;
use crate::services::movie::dummy::FakeMovieService;
use crate::services::movie::movie_service::AsyncMovieService;
use std::sync::Arc;

pub fn create_movie_service(provider: MovieProvider) -> Arc<dyn AsyncMovieService> {
    match provider {
        MovieProvider::Dummy => Arc::new(FakeMovieService),
    }
}
