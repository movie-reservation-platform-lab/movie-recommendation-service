mod di;
mod domain;
mod services;

use crate::di::movie_service::create_movie_service;
use crate::services::movie::movie_service::AsyncMovieService;
use axum::{routing::get, Json, Router};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

///
///
/// TODO: add inversion of control to select the right service
///
#[tokio::main]
async fn main() {
    let app = Router::new().route("/movies", get(get_movies));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Server running on http://{addr}");

    axum::serve(listener, app).await.unwrap();
}

async fn get_movies() -> Json<serde_json::Value> {
    let movie_service: Arc<dyn AsyncMovieService> = create_movie_service();
    let movies = movie_service
        .get_random_movies(5)
        .await
        .expect("Should return a valid movie set");

    Json(serde_json::json!(movies))
}
