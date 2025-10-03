mod config;
mod discovery;
mod health;
mod rest_routes;
mod routing;
mod ws_routes;

use std::net::SocketAddr;

use anyhow::Context;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::config::GatewayConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    if let Err(err) = logline_core::logging::init_tracing(None) {
        eprintln!("⚠️ failed to initialise tracing: {err}");
    }

    let config = GatewayConfig::from_env().context("failed to load gateway configuration")?;
    let app = routing::GatewayApp::new(&config);
    app.mesh.spawn();

    let addr: SocketAddr = config
        .bind_address()
        .parse()
        .context("invalid bind address")?;
    let listener = TcpListener::bind(addr)
        .await
        .context("failed to bind TCP listener")?;
    let actual_addr = listener
        .local_addr()
        .context("failed to read socket address")?;
    info!(%actual_addr, "starting logline-gateway");

    if let Err(err) = axum::serve(listener, app.router.into_make_service()).await {
        error!(?err, "gateway server terminated with error");
    }

    Ok(())
}
