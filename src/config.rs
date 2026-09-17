use anyhow::{bail, Context, Result};
use axum::http::Uri;
use std::env;

pub(crate) const DEFAULT_PORT: u16 = 8082;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MovieProvider {
    Dummy,
}

impl MovieProvider {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Dummy => "dummy",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppConfig {
    pub(crate) port: u16,
    pub(crate) movie_provider: MovieProvider,
    pub(crate) otlp_endpoint: Option<String>,
}

impl AppConfig {
    pub(crate) fn from_env() -> Result<Self> {
        let port = optional_env("PORT")?;
        let use_dummy = optional_env("USE_DUMMY")?;
        let otlp_endpoint = optional_env("OTEL_EXPORTER_OTLP_ENDPOINT")?;

        Self::from_values(
            port.as_deref(),
            use_dummy.as_deref(),
            otlp_endpoint.as_deref(),
        )
    }

    fn from_values(
        port: Option<&str>,
        use_dummy: Option<&str>,
        otlp_endpoint: Option<&str>,
    ) -> Result<Self> {
        let port = match port {
            Some(value) => {
                let port = value
                    .parse::<u16>()
                    .with_context(|| "PORT must be an integer from 1 through 65535")?;
                if port == 0 {
                    bail!("PORT must be an integer from 1 through 65535");
                }
                port
            }
            None => DEFAULT_PORT,
        };

        let movie_provider = match use_dummy.map(str::trim) {
            None => MovieProvider::Dummy,
            Some(value) if value.eq_ignore_ascii_case("true") => MovieProvider::Dummy,
            Some(_) => bail!("USE_DUMMY must be true; no external movie provider is configured"),
        };

        let otlp_endpoint = validated_otlp_endpoint(otlp_endpoint)?;

        Ok(Self {
            port,
            movie_provider,
            otlp_endpoint,
        })
    }
}

fn validated_otlp_endpoint(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        bail!("OTEL_EXPORTER_OTLP_ENDPOINT must not be empty");
    }

    let uri = value
        .parse::<Uri>()
        .context("OTEL_EXPORTER_OTLP_ENDPOINT must be an absolute HTTP(S) URI")?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.authority().is_none() {
        bail!("OTEL_EXPORTER_OTLP_ENDPOINT must be an absolute HTTP(S) URI");
    }
    if uri
        .authority()
        .is_some_and(|authority| authority.as_str().contains('@'))
    {
        bail!("OTEL_EXPORTER_OTLP_ENDPOINT must not contain credentials");
    }
    if uri.query().is_some() {
        bail!("OTEL_EXPORTER_OTLP_ENDPOINT must not contain a query string");
    }

    Ok(Some(value.trim_end_matches('/').to_owned()))
}

fn optional_env(name: &'static str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => bail!("{name} must contain valid Unicode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_stable() {
        assert_eq!(
            AppConfig::from_values(None, None, None).unwrap(),
            AppConfig {
                port: DEFAULT_PORT,
                movie_provider: MovieProvider::Dummy,
                otlp_endpoint: None,
            }
        );
    }

    #[test]
    fn accepts_explicit_supported_values() {
        assert_eq!(
            AppConfig::from_values(
                Some("9090"),
                Some(" TRUE "),
                Some("http://otel-collector:4318"),
            )
            .unwrap(),
            AppConfig {
                port: 9090,
                movie_provider: MovieProvider::Dummy,
                otlp_endpoint: Some("http://otel-collector:4318".into()),
            }
        );
    }

    #[test]
    fn rejects_invalid_ports() {
        for port in ["0", "not-a-port", "65536"] {
            let error = AppConfig::from_values(Some(port), None, None).unwrap_err();
            assert!(error.to_string().contains("PORT"), "port: {port}");
        }
    }

    #[test]
    fn rejects_unsupported_provider_configuration() {
        for use_dummy in ["false", "provider", ""] {
            let error = AppConfig::from_values(None, Some(use_dummy), None).unwrap_err();
            assert!(error.to_string().contains("USE_DUMMY"));
        }
    }

    #[test]
    fn validates_otlp_base_endpoint() {
        for endpoint in [
            "http://otel-collector:4318",
            "https://telemetry.example.test/otlp/",
        ] {
            assert!(validated_otlp_endpoint(Some(endpoint)).is_ok());
        }

        for endpoint in [
            "",
            "otel-collector:4318",
            "file:///tmp/telemetry",
            "http://user:password@collector:4318",
            "https://collector:4318?token=secret",
        ] {
            assert!(
                validated_otlp_endpoint(Some(endpoint)).is_err(),
                "endpoint: {endpoint}"
            );
        }
    }
}
