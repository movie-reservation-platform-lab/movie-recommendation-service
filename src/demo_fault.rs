#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FaultMode {
    #[default]
    None,
    SlowRecommendation,
    RecommendationError,
}

impl FaultMode {
    pub(crate) fn from_value(value: &str) -> Self {
        match value.trim() {
            value if value.eq_ignore_ascii_case("slow-recommendation") => Self::SlowRecommendation,
            value if value.eq_ignore_ascii_case("recommendation-error") => {
                Self::RecommendationError
            }
            _ => Self::None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SlowRecommendation => "slow-recommendation",
            Self::RecommendationError => "recommendation-error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_allowlisted_faults() {
        let cases = [
            ("none", FaultMode::None),
            (" slow-recommendation ", FaultMode::SlowRecommendation),
            ("RECOMMENDATION-ERROR", FaultMode::RecommendationError),
            ("unknown", FaultMode::None),
            ("", FaultMode::None),
        ];

        for (value, expected) in cases {
            assert_eq!(FaultMode::from_value(value), expected, "value: {value}");
        }
    }
}
