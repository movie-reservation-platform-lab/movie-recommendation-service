use crate::services::movie::dummy::FakeMovieService;
use crate::services::movie::movie_service::AsyncMovieService;
use std::sync::Arc;

pub fn create_movie_service() -> Arc<dyn AsyncMovieService> {
    let use_dummy = std::env::var("USE_DUMMY")
        .unwrap_or_else(|_| "true".into())
        .to_lowercase()
        == "true";

    if !use_dummy {
        panic!("Only USE_DUMMY=true is supported for the demo recommendation service");
    }

    Arc::new(FakeMovieService {})
}
