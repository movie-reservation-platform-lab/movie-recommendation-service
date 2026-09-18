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

use crate::domain::recommendation::RatingCalibration;

const SNAPSHOTS: [RatingCalibration; 20] = [
    RatingCalibration {
        minimum: 0.0,
        maximum: 10.0,
    },
    RatingCalibration {
        minimum: 8.0,
        maximum: 8.0,
    },
    RatingCalibration {
        minimum: 1.0,
        maximum: 10.0,
    },
    RatingCalibration {
        minimum: 7.5,
        maximum: 7.5,
    },
    RatingCalibration {
        minimum: 9.0,
        maximum: 9.0,
    },
    RatingCalibration {
        minimum: 2.0,
        maximum: 10.0,
    },
    RatingCalibration {
        minimum: 8.5,
        maximum: 8.5,
    },
    RatingCalibration {
        minimum: 0.0,
        maximum: 9.8,
    },
    RatingCalibration {
        minimum: 7.0,
        maximum: 7.0,
    },
    RatingCalibration {
        minimum: 8.2,
        maximum: 8.2,
    },
    RatingCalibration {
        minimum: 1.5,
        maximum: 10.0,
    },
    RatingCalibration {
        minimum: 9.1,
        maximum: 9.1,
    },
    RatingCalibration {
        minimum: 0.0,
        maximum: 9.9,
    },
    RatingCalibration {
        minimum: 8.7,
        maximum: 8.7,
    },
    RatingCalibration {
        minimum: 2.5,
        maximum: 10.0,
    },
    RatingCalibration {
        minimum: 7.8,
        maximum: 7.8,
    },
    RatingCalibration {
        minimum: 8.4,
        maximum: 8.4,
    },
    RatingCalibration {
        minimum: 0.5,
        maximum: 10.0,
    },
    RatingCalibration {
        minimum: 9.3,
        maximum: 9.3,
    },
    RatingCalibration {
        minimum: 3.0,
        maximum: 10.0,
    },
];

pub(super) fn selected_calibration() -> RatingCalibration {
    sample_calibration(|| uuid::Uuid::new_v4().as_bytes()[0])
}

fn sample_calibration(mut next_byte: impl FnMut() -> u8) -> RatingCalibration {
    // Reject the incomplete final bucket to keep each snapshot equally likely.
    let bucket_end = 256 / SNAPSHOTS.len() * SNAPSHOTS.len();
    loop {
        let value = usize::from(next_byte());
        if value < bucket_end {
            return SNAPSHOTS[value % SNAPSHOTS.len()];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::recommendation::rank_movies;

    #[test]
    fn snapshot_records_produce_both_ranking_outcomes() {
        let results: Vec<_> = SNAPSHOTS
            .into_iter()
            .map(|calibration| rank_movies(seed_movies(), 5, None, calibration))
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 9);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 11);
        for recommendations in results.into_iter().flatten() {
            assert_eq!(recommendations.len(), 5);
            assert!(recommendations
                .iter()
                .all(|item| item.confidence.is_finite()));
        }
    }

    #[test]
    fn sampling_covers_every_record_equally_and_rejects_tail() {
        let mut counts = [0; 20];
        for value in 0..240u8 {
            counts[usize::from(value) % 20] += 1;
            let calibration = sample_calibration(|| value);
            assert_eq!(
                calibration.minimum,
                SNAPSHOTS[usize::from(value) % 20].minimum
            );
            assert_eq!(
                calibration.maximum,
                SNAPSHOTS[usize::from(value) % 20].maximum
            );
        }
        assert_eq!(counts, [12; 20]);
        let mut bytes = (240..=255u8).chain(std::iter::once(19));
        let calibration = sample_calibration(|| bytes.next().unwrap());
        assert_eq!(calibration.minimum, SNAPSHOTS[19].minimum);
        assert_eq!(bytes.next(), None);
    }

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
