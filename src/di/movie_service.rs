use crate::services::movie::dummy::FakeMovieService;
use crate::services::movie::movie_service::AsyncMovieService;
// use crate::services::
use std::sync::Arc;

pub fn create_movie_service() -> Arc<dyn AsyncMovieService> {
    let use_dummy = std::env::var("USE_DUMMY")
        .unwrap_or_else(|_| "false".into())
        .to_lowercase()
        == "true";

    // if use_dummy {
    //     println!("⚙️ Using DummyMovieService");
    //     Arc::new(DummyMovieService)
    // } else {
    //     let api_key = std::env::var("TMDB_API_KEY").unwrap_or_default();
    //     println!("⚙️ Using TmdbMovieService with API key: {}", api_key);
    //     Arc::new(TmdbMovieService { api_key })
    // }
    if use_dummy {
        println!("⚙️ Using FakeMovieService");
        let movie_service = FakeMovieService {};
        Arc::new(movie_service)
    } else {
        panic!("not supported yet")
    }
}
