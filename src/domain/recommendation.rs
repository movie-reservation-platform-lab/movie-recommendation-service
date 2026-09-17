use super::movie::{Movie, MovieRecommendation};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RatingCalibration {
    pub minimum: f32,
    pub maximum: f32,
}

impl Default for RatingCalibration {
    fn default() -> Self {
        Self {
            minimum: 0.0,
            maximum: 10.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RankingError {
    NonFiniteScore,
}

impl fmt::Display for RankingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteScore => f.write_str("ranking produced a non-finite score"),
        }
    }
}
impl Error for RankingError {}

pub(crate) fn rank_movies(
    movies: Vec<Movie>,
    count: usize,
    preference: Option<&str>,
    calibration: RatingCalibration,
) -> Result<Vec<MovieRecommendation>, RankingError> {
    let preference = preference.map(str::trim).filter(|value| !value.is_empty());
    let mut scored_movies = movies
        .into_iter()
        .map(|movie| {
            let score = recommendation_score(&movie, preference, calibration);
            if !score.is_finite() {
                return Err(RankingError::NonFiniteScore);
            }
            Ok(ScoredMovie { movie, score })
        })
        .collect::<Result<Vec<_>, _>>()?;
    scored_movies.sort_by(compare_scored_movies);
    Ok(scored_movies
        .into_iter()
        .take(count)
        .map(|scored| movie_to_recommendation(scored.movie, preference, scored.score))
        .collect())
}

struct ScoredMovie {
    movie: Movie,
    score: f32,
}

fn movie_to_recommendation(
    movie: Movie,
    preference: Option<&str>,
    score: f32,
) -> MovieRecommendation {
    let reason = match preference {
        Some(preference) => {
            format!(
                "Matches demo preference '{preference}' using rating, runtime, and screening availability hints"
            )
        }
        None => "Ranked by rating, runtime, and screening availability hints".into(),
    };

    MovieRecommendation {
        id: format!("recommendation-{}", movie.id.trim_start_matches("movie-")),
        title: movie.title,
        reason,
        confidence: recommendation_confidence(score),
        movie_reservation_movie_id: movie.movie_reservation_movie_id,
    }
}

fn recommendation_score(
    movie: &Movie,
    preference: Option<&str>,
    calibration: RatingCalibration,
) -> f32 {
    let rating_score =
        (movie.vote_average - calibration.minimum) / (calibration.maximum - calibration.minimum);
    let availability_score = availability_hint(movie);
    let runtime_score = runtime_hint(movie.duration_minutes);
    let preference_score = preference
        .map(|preference| preference_hint(movie, preference))
        .unwrap_or(0.0);

    rating_score + availability_score + runtime_score + preference_score
}

fn availability_hint(movie: &Movie) -> f32 {
    match movie.id.as_str() {
        "movie-the-matrix" => 0.09,
        "movie-empire-strikes-back" => 0.09,
        "movie-the-dark-knight" => 0.08,
        "movie-type-safe-matinee" => 0.08,
        "movie-fargate-at-midnight" => 0.08,
        "movie-shawshank-redemption" => 0.07,
        "movie-star-wars-new-hope" => 0.07,
        "movie-last-deployment" => 0.07,
        "movie-casablanca" => 0.06,
        _ => 0.04,
    }
}

fn runtime_hint(duration_minutes: Option<u16>) -> f32 {
    match duration_minutes {
        Some(duration) if duration <= 105 => 0.05,
        Some(duration) if duration <= 125 => 0.03,
        Some(duration) if duration <= 145 => 0.01,
        _ => 0.0,
    }
}

fn preference_hint(movie: &Movie, preference: &str) -> f32 {
    let preference = preference.to_lowercase();
    let title = movie.title.to_lowercase();
    let overview = movie.overview.to_lowercase();
    let mut score: f32 = 0.0;

    if preference.contains("sci-fi")
        || preference.contains("space")
        || preference.contains("simulation")
    {
        score += keyword_score(
            &title,
            &overview,
            &[
                "matrix",
                "star wars",
                "empire",
                "hope",
                "fargate",
                "galactic",
            ],
        );
    }

    if preference.contains("classic") || preference.contains("old") {
        score += keyword_score(
            &title,
            &overview,
            &["casablanca", "shawshank", "star wars", "wartime"],
        );
    }

    if preference.contains("action") || preference.contains("superhero") {
        score += keyword_score(&title, &overview, &["matrix", "dark knight", "batman"]);
    }

    if preference.contains("thriller") || preference.contains("suspense") {
        score += keyword_score(
            &title,
            &overview,
            &["arlington", "thriller", "dark knight", "fargate"],
        );
    }

    if preference.contains("engineering")
        || preference.contains("demo")
        || preference.contains("platform")
        || preference.contains("deployment")
    {
        score += keyword_score(
            &title,
            &overview,
            &[
                "type-safe",
                "fargate",
                "deployment",
                "platform",
                "compile-time",
            ],
        );
    }

    if preference.contains("short")
        && movie
            .duration_minutes
            .is_some_and(|duration| duration <= 105)
    {
        score += 0.12;
    }

    if preference.contains("family") && movie.rating.as_deref() == Some("PG") {
        score += 0.08;
    }

    score.min(0.35)
}

fn keyword_score(title: &str, overview: &str, keywords: &[&str]) -> f32 {
    let matches = keywords
        .iter()
        .filter(|keyword| title.contains(**keyword) || overview.contains(**keyword))
        .count();

    (matches as f32 * 0.08).min(0.24)
}

fn recommendation_confidence(score: f32) -> f32 {
    score.clamp(0.0, 0.99).mul_add(100.0, 0.0).round() / 100.0
}

fn compare_scored_movies(left: &ScoredMovie, right: &ScoredMovie) -> Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| left.movie.title.cmp(&right.movie.title))
}
