# Authentication audit demo

This is a credential-validation demonstration, not a production login system.
It does not create a session, issue a token, or change recommendation access.
Keep it disabled outside the short-lived demo. There is no lockout, per-user
rate limit, or identity-provider integration.

## Local run

In Bash, enter throwaway credentials without putting the password in shell history:

```bash
read -r -p "Demo username: " DEMO_AUTH_USERNAME
read -r -s -p "Demo password: " DEMO_AUTH_PASSWORD
export DEMO_AUTH_USERNAME DEMO_AUTH_PASSWORD
export DEMO_AUTH_ENABLED=true SERVICE_VERSION=local-audit-demo DEPLOYMENT_ENVIRONMENT=local
cargo run --locked
```

From another terminal, trigger a failed attempt:

```sh
curl -i http://localhost:8082/demo/auth/login \
  -H 'Content-Type: application/json' \
  -H 'X-Correlation-Id: audit-demo-action-001' \
  -H 'X-Request-Id: audit-demo-request-001' \
  --data '{"username":"not-a-user","password":"intentionally-wrong"}'
```

Expect HTTP 401 and `authenticated: false`, `request_id`, and `audit_event_id`.
The matching stdout event has `class_uid: 3002`, `activity_id: 99`,
`type_uid: 300299`, `status_detail: INVALID_CREDENTIALS`, and `user.name: unknown`.
Submitted credentials never become audit identity. A correct credential check
returns 200 with an `AUTHENTICATED` event and the fixed demo identity `demo-user`.
Missing credentials return 401; invalid JSON, unexpected fields, oversized
credentials, or a body above 16 KiB return 400. The disabled endpoint returns 404.

To send the configured credentials from the terminal that has them exported,
without placing them in curl's arguments:

```sh
jq -nc '{username: env.DEMO_AUTH_USERNAME, password: env.DEMO_AUTH_PASSWORD}' |
  curl -i http://localhost:8082/demo/auth/login \
    -H 'Content-Type: application/json' --data-binary @-
```

## Follow one attempt across systems

`audit_event_id` in the response equals OCSF `metadata.uid`. The operational JSON
event `audit.authentication` contains that same ID, request/action IDs, outcome,
and any active trace/span/native AWS IDs. The request span carries `audit.event_id`,
`app.request_id`, `app.correlation_id`, `aws.alb.trace_id`, and
`aws.cloudfront.request_id` attributes. No credentials or request body are logged.

Set `OTEL_EXPORTER_OTLP_ENDPOINT` to your collector's HTTP endpoint, for example
`http://localhost:4318`, to enable actual request spans and export. Audit
`unmapped.platform.trace_id` and `span_id` come from the active span, not from a
made-up identifier or the incoming parent span. A valid W3C `traceparent` is
preserved as parent context. Unsampled requests still emit audits with valid
context IDs, but there may be no exported trace to open. Without an OTel layer,
the audit and response omit the trace ID.

On AWS, the ALB supplies `X-Amzn-Trace-Id`; the audit preserves it as
`aws_alb_trace_id`, which can be matched to the ALB access log. It is a different
identifier from the OTel trace ID. CloudFront's `X-Amz-Cf-Id` is included only if
it actually reaches this service. The current ALB-only deployment does not
invent a CloudFront ID. Native and caller-provided IDs are untrusted correlation
hints, never evidence of who authenticated. See the
[AWS ALB tracing contract](https://docs.aws.amazon.com/elasticloadbalancing/latest/application/load-balancer-request-tracing.html).

Query `metadata.uid` in Athena, then pivot using `unmapped.platform.trace_id`
into X-Ray or `audit_event_id` into CloudWatch Logs. Deployment-specific table,
bucket, and collector details belong to the
[infrastructure repository](https://github.com/movie-reservation-platform-lab/movie-platform-infra).

## Code and delivery boundary

`src/audit/event.rs` builds typed OCSF values; it has no HTTP or AWS dependencies.
`src/audit/config.rs` reads explicit configuration and hashes demo credentials for
constant-time comparison. This hashing is not a production password database.
`src/http/demo_auth.rs` translates HTTP and tracing context at the boundary.
`src/audit/sink.rs` is the replaceable transport adapter. These modules can later
be extracted into a library without moving recommendation logic.

The stdout adapter writes and flushes exactly one compact JSON line. It runs
outside Tokio workers, allows at most 16 concurrent writes, and waits at most
two seconds for each response. Busy, failed, or timed-out writes return 503 with
`authenticated: false`. A timed-out write may still finish later; the event ID
is unchanged. The worker holds its concurrency permit until it finishes.
Shutdown has a bounded wait for blocking workers, so pending records can be lost.

A completed stdout write does **not** acknowledge Firehose acceptance or S3
delivery. FireLens buffering, destination retries, and task termination can
still lose records. This is not an authoritative or lossless audit trail. See
[ECS FireLens buffering](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/firelens-docker-buffer-limit.html).

The checked-in [JSON Schema](contracts/platform-audit-event-v1.schema.json) is
the platform's constrained subset of OCSF 1.3.0 Authentication, not the complete
generated OCSF schema. Its shared fixture is copied byte-for-byte across the
three service languages. Unknown fields, submitted usernames, invalid IDs,
and inconsistent success/failure fields are rejected by contract tests.

## Verify and stop

```sh
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

Stop the local process with Ctrl-C. Disable `DEMO_AUTH_ENABLED` or redeploy the
previous immutable image to remove this demo endpoint. This repository creates
no AWS resources; AWS deployment, retention, and teardown are documented in the
infrastructure repository.
