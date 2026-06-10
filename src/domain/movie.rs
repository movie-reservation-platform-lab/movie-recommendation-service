#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Movie {
    pub id: String,
    pub title: String,
    pub overview: String,
    pub release_date: Option<String>,
    pub vote_average: f32,
    pub movie_reservation_movie_id: Option<String>,
    pub rating: Option<String>,
    pub duration_minutes: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct MovieRecommendation {
    pub id: String,
    pub title: String,
    pub reason: String,
    pub confidence: f32,
    pub movie_reservation_movie_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RecommendationResponse {
    pub recommendations: Vec<MovieRecommendation>,
}
