use std::sync::Arc;

use async_trait::async_trait;
use logline_core::errors::LogLineError;
use logline_core::websocket::{
    peer_from_env, ServiceIdentity, ServiceMeshClient, ServiceMeshClientHandle, ServiceMessage,
    ServiceMessageHandler, WebSocketPeer,
};
use logline_protocol::timeline::{Span, SpanStatus};
use logline_rules::{Decision, RuleEngine};
use serde_json::{json, Map, Value};
use tracing::{debug, info, warn};
use url::Url;

use crate::runtime::EngineHandle;
use crate::EngineServiceConfig;

const TIMELINE_PEER_NAME: &str = "logline-timeline";

/// Initialise the engine WebSocket mesh connections based on the provided configuration.
pub fn start_service_mesh(handle: EngineHandle, config: &EngineServiceConfig) {
    let rules = load_rule_engine(config);
    let peers = collect_peers(config);
    if peers.is_empty() {
        info!("engine service mesh disabled: no peers configured");
        return;
    }

    let handler = Arc::new(EngineMeshHandler::new(handle, rules));
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

fn load_rule_engine(config: &EngineServiceConfig) -> Option<Arc<RuleEngine>> {
    let candidate = config
        .rules_path
        .as_deref()
        .map(str::to_owned)
        .or_else(|| std::env::var("ENGINE_RULES_PATH").ok())
        .or_else(|| std::env::var("RULES_PATH").ok());

    match candidate {
        Some(path) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                warn!("rule engine path configured but empty; skipping local evaluation");
                return None;
            }

            match RuleEngine::from_path(trimmed) {
                Ok(engine) => {
                    let count = engine.rules().len();
                    if count == 0 {
                        warn!(%trimmed, "rule engine loaded with no rules");
                    } else {
                        info!(rule_count = count, %trimmed, "loaded local rule engine");
                    }
                    Some(Arc::new(engine))
                }
                Err(err) => {
                    warn!(%trimmed, ?err, "failed to load local rule definitions");
                    None
                }
            }
        }
        None => {
            debug!("no rule engine configuration provided; engine will skip local evaluation");
            None
        }
    }
}

struct EngineMeshHandler {
    _handle: EngineHandle,
    rules: Option<Arc<RuleEngine>>,
}

impl EngineMeshHandler {
    fn new(handle: EngineHandle, rules: Option<Arc<RuleEngine>>) -> Self {
        Self {
            _handle: handle,
            rules,
        }
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
                if metadata.is_null() {
                    debug!(peer = %peer.name, %span_id, "span metadata not provided");
                } else {
                    debug!(peer = %peer.name, %span_id, metadata = ?metadata, "span metadata received");
                }

                if tenant_id.is_none() {
                    warn!(peer = %peer.name, %span_id, "span lacks tenant identifier; proceeding without tenant context");
                }

                if let Some(engine) = &self.rules {
                    match serde_json::from_value::<Span>(span.clone()) {
                        Ok(mut parsed_span) => {
                            let mut outcome = engine.apply(&mut parsed_span);
                            let decision = outcome.decision.clone();

                            if let Decision::Simulate { note } = &decision {
                                parsed_span.status = SpanStatus::Simulated;
                                if let Some(note) = note {
                                    parsed_span.add_metadata(
                                        "simulation_note",
                                        Value::String(note.clone()),
                                    );
                                }
                            }

                            if !outcome.applied_rules.is_empty() || !outcome.notes.is_empty() {
                                parsed_span.add_metadata(
                                    "rule_engine",
                                    json!({
                                        "rules": outcome.applied_rules.clone(),
                                        "notes": outcome.notes.clone(),
                                    }),
                                );
                            }

                            let mut metadata_updates = Map::new();
                            for (key, value) in &outcome.metadata_updates {
                                metadata_updates.insert(key.clone(), value.clone());
                            }

                            let decision_label = match &decision {
                                Decision::Allow => "allow",
                                Decision::Reject { .. } => "reject",
                                Decision::Simulate { .. } => "simulate",
                            };

                            info!(
                                peer = %peer.name,
                                %span_id,
                                tenant = tenant_id.as_deref().unwrap_or("unknown"),
                                decision = decision_label,
                                applied_rules = outcome.applied_rules.len(),
                                "evaluated span locally"
                            );

                            let success = !matches!(decision, Decision::Reject { .. });
                            let output = json!({
                                "decision": decision_label,
                                "applied_rules": outcome.applied_rules,
                                "notes": outcome.notes,
                                "added_tags": outcome.added_tags,
                                "metadata_updates": metadata_updates,
                                "span": parsed_span,
                            });

                            if let Err(err) = client
                                .send_to(
                                    TIMELINE_PEER_NAME,
                                    ServiceMessage::RuleExecutionResult {
                                        result_id: span_id.clone(),
                                        success,
                                        output,
                                    },
                                )
                                .await
                            {
                                warn!(%span_id, ?err, "failed to forward rule execution result to timeline");
                            }
                        }
                        Err(err) => {
                            warn!(peer = %peer.name, %span_id, ?err, "failed to decode span for evaluation");
                        }
                    }
                } else {
                    debug!(peer = %peer.name, %span_id, "no local rule engine configured; skipping evaluation");
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
