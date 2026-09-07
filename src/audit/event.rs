use super::config::EventIdentity;
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Authenticated,
    InvalidCredentials,
    MissingCredentials,
    MalformedCredentials,
}

impl Outcome {
    pub(crate) const fn authenticated(self) -> bool {
        matches!(self, Self::Authenticated)
    }

    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::Authenticated => "AUTHENTICATED",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::MissingCredentials => "MISSING_CREDENTIALS",
            Self::MalformedCredentials => "MALFORMED_CREDENTIALS",
        }
    }
}

/// Values are validated here even when supplied by a future non-HTTP adapter.
pub(crate) struct AuditContext {
    pub(crate) correlation_id: String,
    pub(crate) request_id: String,
    pub(crate) trace_id: Option<String>,
    pub(crate) span_id: Option<String>,
    pub(crate) aws_alb_trace_id: Option<String>,
    pub(crate) aws_cloudfront_request_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AuthenticationEvent {
    activity_id: u8,
    activity_name: &'static str,
    category_uid: u8,
    class_uid: u16,
    type_uid: u32,
    severity_id: u8,
    status_id: u8,
    status_detail: &'static str,
    time: u64,
    metadata: Metadata,
    service: Service,
    user: User,
    unmapped: Unmapped,
}

#[derive(Clone, Debug, Serialize)]
struct Metadata {
    version: &'static str,
    uid: String,
    correlation_uid: String,
    product: Product,
}

#[derive(Clone, Debug, Serialize)]
struct Product {
    name: &'static str,
    vendor_name: &'static str,
    version: String,
}

#[derive(Clone, Debug, Serialize)]
struct Service {
    name: &'static str,
    version: String,
}

#[derive(Clone, Debug, Serialize)]
struct User {
    name: &'static str,
    type_id: u8,
}

#[derive(Clone, Debug, Serialize)]
struct Unmapped {
    platform: Platform,
}

#[derive(Clone, Debug, Serialize)]
struct Platform {
    schema_version: &'static str,
    environment: String,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    span_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aws_alb_trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aws_cloudfront_request_id: Option<String>,
    route: &'static str,
    auth_boundary: &'static str,
}

impl AuthenticationEvent {
    pub(crate) fn new(
        outcome: Outcome,
        identity: &EventIdentity,
        context: AuditContext,
        event_id: Uuid,
        time: u64,
    ) -> Result<Self, &'static str> {
        if !safe_id(&context.request_id) || !safe_id(&context.correlation_id) {
            return Err("invalid audit request or correlation ID");
        }
        if event_id.get_version_num() != 4 || event_id.get_variant() != uuid::Variant::RFC4122 {
            return Err("audit event ID must be UUID v4");
        }
        if context
            .trace_id
            .as_deref()
            .is_some_and(|id| !hex_id(id, 32))
            || context.span_id.as_deref().is_some_and(|id| !hex_id(id, 16))
            || context
                .aws_alb_trace_id
                .as_deref()
                .is_some_and(|id| !safe_native_id(id))
            || context
                .aws_cloudfront_request_id
                .as_deref()
                .is_some_and(|id| !safe_native_id(id))
        {
            return Err("invalid optional audit correlation context");
        }
        // Also validate identity at the event boundary, independent of configuration.
        EventIdentity::from_values(Some(&identity.version), Some(&identity.environment))
            .map_err(|_| "invalid audit service identity")?;
        let success = outcome.authenticated();
        Ok(Self {
            activity_id: 99,
            activity_name: "Credential validation",
            category_uid: 3,
            class_uid: 3002,
            type_uid: 300299,
            severity_id: if success { 1 } else { 2 },
            status_id: if success { 1 } else { 2 },
            status_detail: outcome.reason(),
            time,
            metadata: Metadata {
                version: "1.3.0",
                uid: event_id.to_string(),
                correlation_uid: context.correlation_id,
                product: Product {
                    name: crate::SERVICE_NAME,
                    vendor_name: "Movie Reservation Platform Lab",
                    version: identity.version.clone(),
                },
            },
            service: Service {
                name: crate::SERVICE_NAME,
                version: identity.version.clone(),
            },
            user: User {
                name: if success { "demo-user" } else { "unknown" },
                type_id: u8::from(success),
            },
            unmapped: Unmapped {
                platform: Platform {
                    schema_version: "1",
                    environment: identity.environment.clone(),
                    request_id: context.request_id,
                    trace_id: context.trace_id,
                    span_id: context.span_id,
                    aws_alb_trace_id: context.aws_alb_trace_id,
                    aws_cloudfront_request_id: context.aws_cloudfront_request_id,
                    route: "/demo/auth/login",
                    auth_boundary: "demo_login",
                },
            },
        })
    }
}

pub(crate) fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/@-".contains(&byte))
}

pub(crate) fn safe_native_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

fn hex_id(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::Value;

    pub(crate) fn context() -> AuditContext {
        AuditContext {
            correlation_id: "action-1".into(),
            request_id: "request-1".into(),
            trace_id: None,
            span_id: None,
            aws_alb_trace_id: None,
            aws_cloudfront_request_id: None,
        }
    }

    pub(crate) fn event() -> AuthenticationEvent {
        AuthenticationEvent::new(
            Outcome::InvalidCredentials,
            &EventIdentity::from_values(None, None).unwrap(),
            context(),
            Uuid::new_v4(),
            1788814800000,
        )
        .unwrap()
    }

    #[test]
    fn shared_example_is_valid_and_rejects_contract_drift() {
        let contract: Value =
            serde_json::from_str(include_str!("../../docs/contracts/platform-audit-v1.json"))
                .unwrap();
        let schema: Value = serde_json::from_str(include_str!(
            "../../docs/contracts/platform-audit-event-v1.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&contract["event"]));
        assert!(validator.is_valid(&serde_json::to_value(event()).unwrap()));
        let mut event = contract["event"].clone();
        event["password"] = Value::String("never-store".into());
        assert!(!validator.is_valid(&event));
        let mut event = contract["event"].clone();
        event["user"]["name"] = Value::String("submitted-name".into());
        assert!(!validator.is_valid(&event));
        let mut event = contract["event"].clone();
        event["status_id"] = Value::from(1);
        assert!(!validator.is_valid(&event));
    }

    #[test]
    fn invalid_context_is_rejected_at_the_pure_event_boundary() {
        let identity = EventIdentity::from_values(None, None).unwrap();
        for value in ["", "space id", "plus+id", "tab\tid", &"a".repeat(129)] {
            let mut context = context();
            context.request_id = value.into();
            assert!(AuthenticationEvent::new(
                Outcome::InvalidCredentials,
                &identity,
                context,
                Uuid::new_v4(),
                0
            )
            .is_err());
        }
        for value in ["0".repeat(32), "A".repeat(32), "1".repeat(31)] {
            let mut context = context();
            context.trace_id = Some(value);
            assert!(AuthenticationEvent::new(
                Outcome::InvalidCredentials,
                &identity,
                context,
                Uuid::new_v4(),
                0
            )
            .is_err());
        }
        let mut invalid = context();
        invalid.span_id = Some("0".repeat(16));
        assert!(AuthenticationEvent::new(
            Outcome::InvalidCredentials,
            &identity,
            invalid,
            Uuid::new_v4(),
            0
        )
        .is_err());
        for value in ["tab\t", "non-ascii-é", &"a".repeat(513)] {
            let mut context = context();
            context.aws_alb_trace_id = Some(value.into());
            assert!(AuthenticationEvent::new(
                Outcome::InvalidCredentials,
                &identity,
                context,
                Uuid::new_v4(),
                0
            )
            .is_err());
        }
        assert!(AuthenticationEvent::new(
            Outcome::InvalidCredentials,
            &identity,
            context(),
            Uuid::nil(),
            0
        )
        .is_err());
    }

    #[test]
    fn retries_preserve_event_identity_and_omitted_context_stays_absent() {
        let event = event();
        let original = serde_json::to_value(&event).unwrap();
        assert_eq!(original, serde_json::to_value(event.clone()).unwrap());
        for key in [
            "trace_id",
            "span_id",
            "aws_alb_trace_id",
            "aws_cloudfront_request_id",
        ] {
            assert!(original["unmapped"]["platform"].get(key).is_none());
        }
    }
}
