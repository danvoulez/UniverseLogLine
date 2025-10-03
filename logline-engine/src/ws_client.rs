use std::sync::Arc;

use async_trait::async_trait;
use logline_core::errors::LogLineError;
use logline_core::websocket::{
    peer_from_env, ServiceIdentity, ServiceMeshClient, ServiceMeshClientHandle, ServiceMessage,
    ServiceMessageHandler, WebSocketPeer,
};
use tracing::{debug, info, warn};
use url::Url;

use crate::runtime::EngineHandle;
use crate::EngineServiceConfig;

const TIMELINE_PEER_NAME: &str = "logline-timeline";
const RULES_PEER_NAME: &str = "logline-rules";

/// Initialise the engine WebSocket mesh connections based on the provided configuration.
pub fn start_service_mesh(handle: EngineHandle, config: &EngineServiceConfig) {
    let peers = collect_peers(config);
    if peers.is_empty() {
        info!("engine service mesh disabled: no peers configured");
        return;
    }

    let handler = Arc::new(EngineMeshHandler::new(handle));
    let identity = ServiceIdentity::new(
        "logline-engine",
        vec![
            "task_scheduler".to_string(),
            "span_consumer".to_string(),
            "rule_dispatch".to_string(),
        ],
    );
    let client = Arc::new(ServiceMeshClient::new(identity, peers, handler));
    let runner = Arc::clone(&client);
    runner.spawn();
}

fn collect_peers(config: &EngineServiceConfig) -> Vec<WebSocketPeer> {
    let mut peers = Vec::new();

    if let Some(peer) = peer_from_config(config.timeline_ws_url.as_deref(), TIMELINE_PEER_NAME) {
        peers.push(peer);
    } else if let Ok(Some(peer)) = peer_from_env("TIMELINE_WS_URL", TIMELINE_PEER_NAME) {
        peers.push(peer);
    }

    if let Some(peer) = peer_from_config(config.rules_ws_url.as_deref(), RULES_PEER_NAME) {
        peers.push(peer);
    } else if let Ok(Some(peer)) = peer_from_env("RULES_WS_URL", RULES_PEER_NAME) {
        peers.push(peer);
    }

    peers
}

fn peer_from_config(value: Option<&str>, name: &str) -> Option<WebSocketPeer> {
    value.and_then(|url| {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return None;
        }

        match Url::parse(trimmed) {
            Ok(_) => Some(WebSocketPeer::new(name.to_string(), trimmed.to_string())),
            Err(err) => {
                warn!(%name, %trimmed, ?err, "invalid WebSocket URL in configuration");
                None
            }
        }
    })
}

struct EngineMeshHandler {
    _handle: EngineHandle,
}

impl EngineMeshHandler {
    fn new(handle: EngineHandle) -> Self {
        Self { _handle: handle }
    }
}

#[async_trait]
impl ServiceMessageHandler for EngineMeshHandler {
    fn identity(&self) -> ServiceIdentity {
        ServiceIdentity::new(
            "logline-engine",
            vec![
                "task_scheduler".to_string(),
                "span_consumer".to_string(),
                "rule_dispatch".to_string(),
            ],
        )
    }

    async fn handle_connection_established(
        &self,
        _client: ServiceMeshClientHandle,
        peer: &WebSocketPeer,
    ) -> Result<(), LogLineError> {
        info!(peer = %peer.name, "engine connected to service peer");
        Ok(())
    }

    async fn handle_message(
        &self,
        client: ServiceMeshClientHandle,
        peer: &WebSocketPeer,
        message: ServiceMessage,
    ) -> Result<(), LogLineError> {
        match message {
            ServiceMessage::SpanCreated {
                span_id,
                tenant_id,
                span,
                metadata,
            } => {
                info!(peer = %peer.name, %span_id, "received span via mesh");
                if let Some(tenant) = tenant_id {
                    let request = ServiceMessage::RuleEvaluationRequest {
                        request_id: span_id.clone(),
                        tenant_id: tenant.clone(),
                        span,
                    };
                    if let Err(err) = client.send_to(RULES_PEER_NAME, request).await {
                        warn!(%span_id, %tenant, ?err, "failed to dispatch rule evaluation request");
                    }
                } else {
                    warn!(peer = %peer.name, %span_id, "span lacks tenant identifier; skipping rule evaluation");
                }

                if !metadata.is_null() {
                    debug!(peer = %peer.name, %span_id, metadata = ?metadata, "span metadata received");
                }
            }
            ServiceMessage::RuleExecutionResult {
                result_id,
                success,
                output,
            } => {
                if success {
                    info!(peer = %peer.name, %result_id, "rule execution succeeded");
                } else {
                    warn!(peer = %peer.name, %result_id, output = ?output, "rule execution failed");
                }
            }
            ServiceMessage::ServiceHello {
                sender,
                capabilities,
            } => {
                info!(peer = %peer.name, %sender, ?capabilities, "service handshake acknowledged");
            }
            ServiceMessage::ConnectionLost { peer: lost_peer } => {
                warn!(peer = %peer.name, %lost_peer, "peer reported lost connection");
            }
            other => {
                debug!(peer = %peer.name, message = ?other, "unhandled mesh message");
            }
        }

        Ok(())
    }

    async fn handle_connection_lost(&self, peer: &WebSocketPeer) -> Result<(), LogLineError> {
        warn!(peer = %peer.name, "engine mesh connection closed");
        Ok(())
    }
}
