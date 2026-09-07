# Correlated authentication audit demo

Tracking: [issue #6](https://github.com/movie-reservation-platform-lab/movie-recommendation-service/issues/6).
Integration: [infra #43](https://github.com/movie-reservation-platform-lab/movie-platform-infra/issues/43).

## Scope and boundaries

The service currently has public recommendation routes and explicit request spans,
but no credential validator. Add an opt-in `/demo/auth/login` credential check;
leave `/health`, `/ready`, `/movies`, `/recommendations`, and demo faults alone.
This endpoint issues no session, cookie, or token and protects no existing route.

`src/audit/event.rs` owns the constrained OCSF 1.3 Authentication event. It accepts
typed, bounded context; it does not import Axum, tracing, or an AWS SDK.
`src/audit/sink.rs` writes one compact `{"audit":EVENT}` JSON line to stdout.
`src/http/demo_auth.rs` maps the HTTP request, active span, and credential result.
`src/audit/config.rs` validates explicit credentials and artifact/environment identity.

The alternative was calling Firehose directly from the handler. That would expose
an AWS acceptance boundary but couple this service to AWS delivery. The selected
stdout path keeps transport in FireLens; successful writes do **not** prove S3
delivery. This is an operational audit demo, not a compliance-grade evidence store.

## Implementation

1. Add bounded typed events and byte-identical shared contract fixtures. Use UUID
   v4 event IDs; never retain the submitted username or password in an event.
2. Add opt-in config and a small constant-time verifier. Hash inputs to fixed-size
   values with SHA-256 and compare using `subtle`; these are not stored passwords
   for a production identity system. Add `uuid`, `sha2`, and `subtle` for these
   specific jobs; use JSON Schema validation only in tests.
3. Merge the demo router in `main.rs`. Wrong/missing credentials return 401,
   malformed input 400, correct credentials 200, disabled endpoint 404. Missing
   enabled credentials fail startup. Sink failure returns 503, never success.
4. Await the stdout write outside Tokio workers, with bounded concurrency and a
   finite response timeout. A timed-out blocking write can still finish later;
   keep its admission permit until it finishes, preventing unbounded workers.
5. Copy only active OTel trace/span IDs. Capture bounded ALB/CloudFront headers as
   separate untrusted hints; never pretend they equal the OTel ID. Put event ID
   and request/AWS context on the span and a safe operational event for lookup.
6. Add service version/environment to OTel resources and document local execution.

## Verification and rollback

Tests exercise the actual in-process router, exported request spans, unsampled
context, concurrent request isolation, malformed/oversized bodies, config errors,
redaction, stdout failure, and event-schema conformance. Existing tests remain.
Run format, all-target checks, Clippy with warnings denied, all-target tests, and
a local release binary/container smoke. Do not deploy to AWS from this repo.

Enable only for the demo with explicit environment credentials. Disable
`DEMO_AUTH_ENABLED` or redeploy the previous immutable image to roll back. Keep
this public demo endpoint off outside the short-lived demonstration; it has no
production login controls, account lifecycle, or lockout policy.
