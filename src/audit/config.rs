use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::env;
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub(crate) struct DemoCredentials {
    username_hash: [u8; 32],
    password_hash: [u8; 32],
}

impl DemoCredentials {
    pub(crate) fn from_values(
        enabled: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Option<Self>> {
        match enabled.map(str::trim) {
            None => return Ok(None),
            Some(value) if value.eq_ignore_ascii_case("false") => return Ok(None),
            Some(value) if value.eq_ignore_ascii_case("true") => {}
            Some(_) => bail!("DEMO_AUTH_ENABLED must be true or false"),
        }
        let username = required_credential("DEMO_AUTH_USERNAME", username, 256)?;
        let password = required_credential("DEMO_AUTH_PASSWORD", password, 1024)?;
        Ok(Some(Self {
            username_hash: Sha256::digest(username.as_bytes()).into(),
            password_hash: Sha256::digest(password.as_bytes()).into(),
        }))
    }

    pub(crate) fn from_env() -> Result<Option<Self>> {
        Self::from_values(
            optional_env("DEMO_AUTH_ENABLED")?.as_deref(),
            optional_env("DEMO_AUTH_USERNAME")?.as_deref(),
            optional_env("DEMO_AUTH_PASSWORD")?.as_deref(),
        )
    }

    pub(crate) fn matches(&self, username: &str, password: &str) -> bool {
        let username_hash: [u8; 32] = Sha256::digest(username.as_bytes()).into();
        let password_hash: [u8; 32] = Sha256::digest(password.as_bytes()).into();
        bool::from(
            self.username_hash.ct_eq(&username_hash) & self.password_hash.ct_eq(&password_hash),
        )
    }
}

fn required_credential<'a>(name: &str, value: Option<&'a str>, limit: usize) -> Result<&'a str> {
    match value {
        Some(value) if !value.trim().is_empty() && value.chars().count() <= limit => Ok(value),
        _ => bail!(
            "{name} must be nonblank and at most {limit} characters when demo auth is enabled"
        ),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EventIdentity {
    pub(crate) version: String,
    pub(crate) environment: String,
}

impl EventIdentity {
    pub(crate) fn from_env() -> Result<Self> {
        Self::from_values(
            optional_env("SERVICE_VERSION")?.as_deref(),
            optional_env("DEPLOYMENT_ENVIRONMENT")?.as_deref(),
        )
    }

    pub(crate) fn from_values(version: Option<&str>, environment: Option<&str>) -> Result<Self> {
        Ok(Self {
            version: identity_value(
                "SERVICE_VERSION",
                version.unwrap_or(env!("CARGO_PKG_VERSION")),
            )?,
            environment: identity_value("DEPLOYMENT_ENVIRONMENT", environment.unwrap_or("local"))?,
        })
    }
}

fn identity_value(name: &str, value: &str) -> Result<String> {
    if value.trim().is_empty() || value.chars().count() > 128 || value.chars().any(char::is_control)
    {
        bail!("{name} must be nonblank, at most 128 characters, and contain no control characters");
    }
    Ok(value.to_owned())
}

fn optional_env(name: &str) -> Result<Option<String>> {
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
    fn credentials_are_opt_in_and_required_without_disclosing_values() {
        assert!(DemoCredentials::from_values(None, None, None)
            .unwrap()
            .is_none());
        assert!(DemoCredentials::from_values(Some("false"), None, None)
            .unwrap()
            .is_none());
        assert!(DemoCredentials::from_values(Some("yes"), None, None).is_err());
        for (username, password) in [
            (None, None),
            (Some("test-user"), None),
            (Some(" "), Some("secret-marker")),
            (Some("test-user"), Some(" ")),
        ] {
            let error = DemoCredentials::from_values(Some("true"), username, password)
                .err()
                .unwrap();
            assert!(!error.to_string().contains("secret-marker"));
        }
    }

    #[test]
    fn compares_both_credentials_without_trimming_or_unicode_confusion() {
        let credentials =
            DemoCredentials::from_values(Some("TRUE"), Some("test-user"), Some(" secret-🔐 "))
                .unwrap()
                .unwrap();
        assert!(credentials.matches("test-user", " secret-🔐 "));
        assert!(!credentials.matches("test-user", "secret-🔐"));
        assert!(!credentials.matches("wrong", " secret-🔐 "));
        assert!(!credentials.matches("test-user", "wrong"));
    }

    #[test]
    fn identity_is_bounded_but_allows_semver_build_metadata() {
        assert_eq!(
            EventIdentity::from_values(Some("1.2.3+build.7"), None)
                .unwrap()
                .version,
            "1.2.3+build.7"
        );
        for value in ["", "\ninvalid", &"x".repeat(129)] {
            assert!(EventIdentity::from_values(Some(value), None).is_err());
            assert!(EventIdentity::from_values(None, Some(value)).is_err());
        }
    }
}
