use crate::domain::movie::{Movie, MovieRecommendation};
use crate::services::movie::movie_service::AsyncMovieService;
use anyhow::{Error, Result};
use async_trait::async_trait;
use std::cmp::Ordering;
use tracing::info_span;

pub struct FakeMovieService;

#[async_trait]
impl AsyncMovieService for FakeMovieService {
    async fn get_movies(&self, count: usize) -> Result<Vec<Movie>, Error> {
        Ok(seed_movies().into_iter().take(count).collect())
    }

    async fn get_recommendations(
        &self,
        count: usize,
        preference: Option<String>,
    ) -> Result<Vec<MovieRecommendation>, Error> {
        let preference = preference
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let rank_span = info_span!(
            "recommendations.rank",
            preference.present = preference.is_some(),
            requested.count = count
        );
        let _rank_span = rank_span.enter();

        let mut scored_movies = seed_movies()
            .into_iter()
            .map(|movie| {
                let score = recommendation_score(&movie, preference.as_deref());
                ScoredMovie { movie, score }
            })
            .collect::<Vec<_>>();

        scored_movies.sort_by(compare_scored_movies);

        Ok(scored_movies
            .into_iter()
            .take(count)
            .map(|scored_movie| {
                movie_to_recommendation(
                    scored_movie.movie,
                    preference.as_deref(),
                    scored_movie.score,
                )
            })
            .collect())
    }
}

struct ScoredMovie {
    movie: Movie,
    score: f32,
}

fn seed_movies() -> Vec<Movie> {
    vec![
        Movie {
            id: "movie-shawshank-redemption".into(),
            title: "The Shawshank Redemption".into(),
            overview: "A patient prison drama about endurance, loyalty, and hope.".into(),
            release_date: Some("1994".into()),
            vote_average: 9.6,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444434".into()),
            rating: Some("R".into()),
            duration_minutes: Some(142),
        },
        Movie {
            id: "movie-the-matrix".into(),
            title: "The Matrix".into(),
            overview: "A hacker discovers reality is a simulation and joins a rebellion.".into(),
            release_date: Some("1999".into()),
            vote_average: 9.5,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444435".into()),
            rating: Some("R".into()),
            duration_minutes: Some(136),
        },
        Movie {
            id: "movie-empire-strikes-back".into(),
            title: "Star Wars: Episode V - The Empire Strikes Back".into(),
            overview: "The rebellion scatters while old secrets reshape a galactic conflict."
                .into(),
            release_date: Some("1980".into()),
            vote_average: 9.3,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444436".into()),
            rating: Some("PG".into()),
            duration_minutes: Some(124),
        },
        Movie {
            id: "movie-star-wars-new-hope".into(),
            title: "Star Wars: Episode IV - A New Hope".into(),
            overview: "A farm boy joins a rebellion and helps challenge a planet-killing empire."
                .into(),
            release_date: Some("1977".into()),
            vote_average: 9.1,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444437".into()),
            rating: Some("PG".into()),
            duration_minutes: Some(121),
        },
        Movie {
            id: "movie-the-dark-knight".into(),
            title: "The Dark Knight".into(),
            overview: "A crime saga where Batman faces an adversary built on chaos.".into(),
            release_date: Some("2008".into()),
            vote_average: 9.4,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444438".into()),
            rating: Some("PG-13".into()),
            duration_minutes: Some(152),
        },
        Movie {
            id: "movie-arlington-road".into(),
            title: "Arlington Road".into(),
            overview: "A paranoid thriller about a professor suspecting a dangerous neighbor."
                .into(),
            release_date: Some("1999".into()),
            vote_average: 8.3,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444439".into()),
            rating: Some("R".into()),
            duration_minutes: Some(117),
        },
        Movie {
            id: "movie-type-safe-matinee".into(),
            title: "The Type-Safe Matinee".into(),
            overview: "A demo-friendly programming comedy about compile-time confidence.".into(),
            release_date: Some("2026".into()),
            vote_average: 8.7,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444441".into()),
            rating: Some("PG".into()),
            duration_minutes: Some(102),
        },
        Movie {
            id: "movie-fargate-at-midnight".into(),
            title: "Fargate at Midnight".into(),
            overview: "A platform thriller about containers, alerts, and one late deployment."
                .into(),
            release_date: Some("2026".into()),
            vote_average: 8.6,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444442".into()),
            rating: Some("PG-13".into()),
            duration_minutes: Some(118),
        },
        Movie {
            id: "movie-last-deployment".into(),
            title: "The Last Deployment".into(),
            overview: "An operational drama about shipping carefully when everyone is watching."
                .into(),
            release_date: Some("2026".into()),
            vote_average: 8.4,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444443".into()),
            rating: Some("PG".into()),
            duration_minutes: Some(96),
        },
        Movie {
            id: "movie-casablanca".into(),
            title: "Casablanca".into(),
            overview: "A wartime romance about loyalty, sacrifice, and impossible choices.".into(),
            release_date: Some("1942".into()),
            vote_average: 9.2,
            movie_reservation_movie_id: Some("44444444-4444-4444-8444-444444444444".into()),
            rating: Some("PG".into()),
            duration_minutes: Some(102),
        },
    ]
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

fn recommendation_score(movie: &Movie, preference: Option<&str>) -> f32 {
    let rating_score = movie.vote_average / 10.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_service_contains_sibling_demo_seed_titles() {
        let service = FakeMovieService;

        let movies = service.get_movies(20).await.unwrap();
        let titles = movies
            .iter()
            .map(|movie| movie.title.as_str())
            .collect::<Vec<_>>();

        assert_eq!(movies.len(), 10);
        assert!(titles.contains(&"The Shawshank Redemption"));
        assert!(titles.contains(&"The Matrix"));
        assert!(titles.contains(&"The Type-Safe Matinee"));
        assert!(titles.contains(&"Fargate at Midnight"));
        assert!(titles.contains(&"The Last Deployment"));
        assert!(titles.contains(&"Casablanca"));
    }

    #[tokio::test]
    async fn recommendations_include_reservation_movie_ids() {
        let service = FakeMovieService;

        let recommendations = service
            .get_recommendations(3, Some("sci-fi".into()))
            .await
            .unwrap();

        assert_eq!(recommendations.len(), 3);
        assert!(recommendations
            .iter()
            .any(|recommendation| recommendation.title == "The Matrix"));
        assert!(recommendations[0].reason.contains("sci-fi"));
    }

    #[tokio::test]
    async fn recommendations_rank_by_preference_heuristic() {
        let service = FakeMovieService;

        let recommendations = service
            .get_recommendations(2, Some("platform deployment demo".into()))
            .await
            .unwrap();

        let titles = recommendations
            .iter()
            .map(|recommendation| recommendation.title.as_str())
            .collect::<Vec<_>>();

        assert!(titles.contains(&"Fargate at Midnight"));
        assert!(
            titles.contains(&"The Last Deployment") || titles.contains(&"The Type-Safe Matinee")
        );
    }
}
