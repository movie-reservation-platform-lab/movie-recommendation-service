use crate::domain::movie::Movie;

pub(super) fn seed_movies() -> Vec<Movie> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::recommendation::{rank_movies, RatingCalibration};

    #[test]
    fn baseline_calibration_preserves_ranking() {
        let recommendations = rank_movies(
            seed_movies(),
            2,
            Some("sci-fi"),
            RatingCalibration::default(),
        )
        .unwrap();
        assert_eq!(
            recommendations[0].title,
            "Star Wars: Episode V - The Empire Strikes Back"
        );
    }
}
