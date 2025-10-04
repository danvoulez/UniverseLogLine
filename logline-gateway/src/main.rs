mod config;
mod discovery;
mod health;
mod onboarding;
mod rate_limit;
mod resilience;
mod rest_routes;
mod routing;
mod security;
mod ws_routes;

use std::net::SocketAddr;

use anyhow::Context;
use tracing::{error, info};

use crate::config::GatewayConfig;
use tower::make::Shared;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    if let Err(err) = logline_core::logging::init_tracing(None) {
        eprintln!("⚠️ failed to initialise tracing: {err}");
    }

    let config = GatewayConfig::from_env().context("failed to load gateway configuration")?;
    let app = routing::GatewayApp::new(&config);
    let mesh = app.mesh;
    mesh.spawn();

    let mut router = Some(app.router);

    let addr: SocketAddr = config
        .bind_address()
        .parse()
        .context("invalid bind address")?;

    if let Some(tls) = config.tls() {
        let tls_config = tls
            .load()
            .await
            .context("failed to load TLS certificates")?;
        info!(%addr, "starting logline-gateway with TLS");
        let handle = axum_server::Handle::new();
        let shutdown = shutdown_signal();
        tokio::spawn({
            let handle = handle.clone();
            async move {
                shutdown.await;
                handle.graceful_shutdown(None);
            }
        });
        let router = router
            .take()
            .expect("router should be available before TLS server start");

        let service = Shared::new(router.into_service());

        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(service)
            .await
            .context("gateway server terminated with TLS error")?;
    } else {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("failed to bind TCP listener")?;
        let actual_addr = listener
            .local_addr()
            .context("failed to read socket address")?;
        info!(%actual_addr, "starting logline-gateway");

        let router = router
            .take()
            .expect("router should be available before TCP server start");

        if let Err(err) = axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
            .await
        {
            error!(?err, "gateway server terminated with error");
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
