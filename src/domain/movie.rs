#[derive(serde::Serialize)]
pub struct Movie {
    pub id: u32,
    pub title: String,
    pub overview: String,
    pub release_date: Option<String>,
    pub vote_average: f32,
}
