mod audit;
mod config;
mod demo_fault;
mod di;
mod domain;
mod http;
mod services;
mod telemetry;

use crate::{config::AppConfig, di::movie_service::create_movie_service, http::build_app};
use anyhow::{Context, Result};
use audit::{
    config::{DemoCredentials, EventIdentity},
    sink::{AuditEmitter, StdoutSink},
};
use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::TcpListener;

pub(crate) const SERVICE_NAME: &str = "movie-recommendation-service";
const TELEMETRY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(run());
    // Stdout is external I/O: a wedged blocking writer must not hang process shutdown forever.
    runtime.shutdown_timeout(TELEMETRY_SHUTDOWN_TIMEOUT);
    result
}

async fn run() -> Result<()> {
    let config = AppConfig::from_env().context("invalid service configuration")?;
    let demo_credentials =
        DemoCredentials::from_env().context("invalid demo authentication configuration")?;
    let event_identity = EventIdentity::from_env().context("invalid service identity")?;
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind recommendation service to {addr}"))?;
    let bound_addr = listener
        .local_addr()
        .context("failed to read bound listener address")?;

    let telemetry = Arc::new(telemetry::init(
        SERVICE_NAME,
        config.otlp_endpoint.as_deref(),
        &event_identity,
    )?);
    let app = build_app(
        create_movie_service(config.movie_provider),
        telemetry.clone(),
        config.default_fault,
        config.allow_request_demo_faults,
    )
    .merge(http::demo_auth::router(
        demo_credentials,
        event_identity,
        AuditEmitter::new(Arc::new(StdoutSink)),
    ));

    telemetry.record_starting(
        &bound_addr.to_string(),
        config.port,
        config.movie_provider.as_str(),
        config.default_fault.as_str(),
        config.allow_request_demo_faults,
    );

    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(telemetry.clone()))
        .await;
    telemetry.shutdown(TELEMETRY_SHUTDOWN_TIMEOUT);

    serve_result.context("recommendation HTTP server failed")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownReason {
    Interrupt,
    Terminate,
}

impl ShutdownReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Interrupt => "interrupt",
            Self::Terminate => "terminate",
        }
    }
}

async fn shutdown_signal(telemetry: Arc<telemetry::Telemetry>) {
    #[cfg(unix)]
    let reason = first_shutdown_signal(ctrl_c_signal(), terminate_signal()).await;

    #[cfg(not(unix))]
    let reason = {
        ctrl_c_signal().await;
        ShutdownReason::Interrupt
    };

    telemetry.record_shutdown_requested(reason.as_str());
}

async fn first_shutdown_signal<C, T>(ctrl_c: C, terminate: T) -> ShutdownReason
where
    C: Future<Output = ()>,
    T: Future<Output = ()>,
{
    tokio::pin!(ctrl_c);
    tokio::pin!(terminate);

    tokio::select! {
        () = &mut ctrl_c => ShutdownReason::Interrupt,
        () = &mut terminate => ShutdownReason::Terminate,
    }
}

async fn ctrl_c_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(
            service_name = SERVICE_NAME,
            event = "service.signal_listener_failed",
            signal = "interrupt",
            error = %error,
            "failed to listen for shutdown signal"
        );
        std::future::pending::<()>().await;
    }
}

#[cfg(unix)]
async fn terminate_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    match signal(SignalKind::terminate()) {
        Ok(mut signal) => {
            if signal.recv().await.is_none() {
                tracing::warn!(
                    service_name = SERVICE_NAME,
                    event = "service.signal_listener_closed",
                    signal = "terminate",
                    "shutdown signal listener closed unexpectedly"
                );
                std::future::pending::<()>().await;
            }
        }
        Err(error) => {
            tracing::warn!(
                service_name = SERVICE_NAME,
                event = "service.signal_listener_failed",
                signal = "terminate",
                error = %error,
                "failed to listen for shutdown signal"
            );
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn interrupt_completes_shutdown_selector() {
        let reason = first_shutdown_signal(std::future::ready(()), std::future::pending()).await;
        assert_eq!(reason, ShutdownReason::Interrupt);
    }

    #[tokio::test]
    async fn terminate_completes_shutdown_selector() {
        let reason = first_shutdown_signal(std::future::pending(), std::future::ready(())).await;
        assert_eq!(reason, ShutdownReason::Terminate);
    }
}
