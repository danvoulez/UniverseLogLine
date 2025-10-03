use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tracing::debug;

use logline_core::websocket::ServiceMeshClientHandle;

use crate::discovery::{ServiceDiscovery, ServiceEndpoint};

#[derive(Clone)]
pub struct HealthState {
    client: reqwest::Client,
    endpoints: Vec<ServiceEndpoint>,
    mesh_handle: ServiceMeshClientHandle,
    expected_mesh: Vec<String>,
}

impl HealthState {
    pub fn new(
        client: reqwest::Client,
        discovery: &ServiceDiscovery,
        mesh_handle: ServiceMeshClientHandle,
    ) -> Self {
        let endpoints = discovery.all();
        let expected_mesh = endpoints
            .iter()
            .filter(|endpoint| endpoint.ws_url.is_some())
            .map(|endpoint| endpoint.service_name.to_string())
            .collect();

        Self {
            client,
            endpoints,
            mesh_handle,
            expected_mesh,
        }
    }
}

#[derive(Serialize)]
struct ServiceHealth {
    key: String,
    service: String,
    url: String,
    healthy: bool,
    message: Option<String>,
}

#[derive(Serialize)]
struct MeshHealth {
    connected: Vec<String>,
    expected: Vec<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    services: Vec<ServiceHealth>,
    mesh: MeshHealth,
}

pub fn router(state: HealthState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .with_state(state)
}

async fn healthz(State(state): State<HealthState>) -> impl IntoResponse {
    let mut services = Vec::new();
    let mut overall_ok = true;

    for endpoint in &state.endpoints {
        let health_url = endpoint.health_url();
        let result = state.client.get(&health_url).send().await;
        match result {
            Ok(response) => {
                let healthy = response.status().is_success();
                if !healthy {
                    overall_ok = false;
                }
                services.push(ServiceHealth {
                    key: endpoint.key.to_string(),
                    service: endpoint.service_name.to_string(),
                    url: health_url,
                    healthy,
                    message: None,
                });
            }
            Err(err) => {
                overall_ok = false;
                services.push(ServiceHealth {
                    key: endpoint.key.to_string(),
                    service: endpoint.service_name.to_string(),
                    url: health_url,
                    healthy: false,
                    message: Some(err.to_string()),
                });
            }
        }
    }

    let connected = state.mesh_handle.connected_peers().await;
    if connected.len() < state.expected_mesh.len() {
        overall_ok = false;
    }

    debug!(connected = ?connected, "estado atual da malha de serviços");

    let response = HealthResponse {
        status: if overall_ok {
            "ok".into()
        } else {
            "degraded".into()
        },
        services,
        mesh: MeshHealth {
            connected,
            expected: state.expected_mesh.clone(),
        },
    };

    Json(response)
}
