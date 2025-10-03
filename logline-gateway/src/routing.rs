use axum::Router;
use tower_http::trace::TraceLayer;

use crate::config::GatewayConfig;
use crate::discovery::ServiceDiscovery;
use crate::health::{router as health_router, HealthState};
use crate::onboarding::{router as onboarding_router, OnboardingState};
use crate::rest_routes::{router as rest_router, RestProxyState};
use crate::ws_routes::{initialise_mesh, router as ws_router, GatewayMesh};

pub struct GatewayApp {
    pub router: Router,
    pub mesh: GatewayMesh,
}

impl GatewayApp {
    pub fn new(config: &GatewayConfig) -> Self {
        let discovery = ServiceDiscovery::from_config(config);
        build_app(discovery)
    }
}

pub fn build_app(discovery: ServiceDiscovery) -> GatewayApp {
    let client = reqwest::Client::new();

    let rest_state = RestProxyState::new(client.clone(), &discovery);
    let rest_router = rest_router(rest_state);

    let onboarding_state = OnboardingState::new(client.clone(), &discovery)
        .expect("onboarding requer serviços de identidade e timeline");
    let onboarding_router = onboarding_router(onboarding_state);

    let (mesh, ws_state) = initialise_mesh(&discovery);
    let ws_router = ws_router(ws_state.clone());

    let health_state = HealthState::new(client, &discovery, ws_state.mesh_handle.clone());
    let health_router = health_router(health_state);

    let router = Router::new()
        .merge(rest_router)
        .merge(onboarding_router)
        .merge(ws_router)
        .merge(health_router)
        .layer(TraceLayer::new_for_http());

    GatewayApp { router, mesh }
}
