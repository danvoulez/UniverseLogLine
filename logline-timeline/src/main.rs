mod repository;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use hyper::Error as HyperError;
use logline_core::config::CoreConfig;
use logline_core::errors::LogLineError;
use logline_core::logging;
use logline_core::websocket::{ServiceMessage, WebSocketEnvelope};
use logline_protocol::timeline::{
    Span, SpanStatus, SpanType, TimelineEntry, TimelineQuery, Visibility,
};
use repository::TimelineRepository;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    if let Err(err) = logging::init_tracing(None) {
        eprintln!("⚠️ failed to initialise tracing: {err}");
    }

    let config = load_timeline_config()?;
    let bind_addr: SocketAddr = config
        .http_bind
        .clone()
        .unwrap_or_else(|| "0.0.0.0:8082".to_string())
        .parse()?;

    let repository = TimelineRepository::from_config(&config).await?;
    let (tx, _rx) = broadcast::channel(128);
    let service_bus = ServiceBus::new();

    let state = AppState {
        repository,
        broadcaster: tx,
        service_bus,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/spans", get(list_spans).post(create_span))
        .route("/v1/spans/:id", get(get_span))
        .route("/ws", get(ws_upgrade))
        .route("/ws/service", get(service_ws_upgrade))
        .with_state(state.clone());

    let listener = TcpListener::bind(bind_addr).await?;
    let actual_addr = listener.local_addr()?;
    info!(%actual_addr, "starting logline-timeline service");
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

fn load_timeline_config() -> Result<CoreConfig, LogLineError> {
    CoreConfig::from_env_with_prefix("TIMELINE_")
        .or_else(|_| CoreConfig::from_env())
        .map_err(Into::into)
}

async fn health_check() -> &'static str {
    "ok"
}

#[derive(Clone)]
struct AppState {
    repository: TimelineRepository,
    broadcaster: broadcast::Sender<TimelineEntry>,
    service_bus: ServiceBus,
}

impl AppState {
    fn subscribe(&self) -> broadcast::Receiver<TimelineEntry> {
        self.broadcaster.subscribe()
    }
}

#[derive(Clone, Default)]
struct ServiceBus {
    inner: Arc<Mutex<HashMap<Uuid, mpsc::UnboundedSender<ServiceMessage>>>>,
}

impl ServiceBus {
    fn new() -> Self {
        Self::default()
    }

    fn register(&self) -> (Uuid, mpsc::UnboundedReceiver<ServiceMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let id = Uuid::new_v4();
        let mut guard = self
            .inner
            .lock()
            .expect("service bus mutex poisoned while registering peer");
        guard.insert(id, tx);
        (id, rx)
    }

    fn unregister(&self, id: Uuid) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.remove(&id);
        }
    }

    fn send_to(&self, id: &Uuid, message: ServiceMessage) -> bool {
        match self.inner.lock() {
            Ok(mut guard) => {
                if let Some(sender) = guard.get(id) {
                    if sender.send(message).is_err() {
                        guard.remove(id);
                        return false;
                    }
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    fn broadcast(&self, message: ServiceMessage) {
        let mut stale = Vec::new();
        if let Ok(guard) = self.inner.lock() {
            for (id, sender) in guard.iter() {
                if sender.send(message.clone()).is_err() {
                    stale.push(*id);
                }
            }
        }

        if let Ok(mut guard) = self.inner.lock() {
            for id in stale {
                guard.remove(&id);
            }
        }
    }
}

type AppResult<T> = Result<T, AppError>;

async fn create_span(
    State(state): State<AppState>,
    Json(payload): Json<CreateSpanRequest>,
) -> AppResult<Json<TimelineEntry>> {
    let span = payload.into_span();
    let span_snapshot = span.clone();

    let entry = state.repository.create_span(span).await?;

    if let Err(err) = state.broadcaster.send(entry.clone()) {
        warn!(?err, "failed to broadcast new span");
    }

    if let Ok(span_json) = serde_json::to_value(&span_snapshot) {
        let metadata = match serde_json::to_value(&entry) {
            Ok(entry_json) => serde_json::json!({ "timeline_entry": entry_json }),
            Err(err) => {
                warn!(
                    ?err,
                    "failed to encode timeline entry for service broadcast"
                );
                serde_json::Value::Null
            }
        };

        let message = ServiceMessage::SpanCreated {
            span_id: span_snapshot.id.to_string(),
            tenant_id: span_snapshot.tenant_id.clone(),
            span: span_json,
            metadata,
        };

        state.service_bus.broadcast(message);
    } else {
        warn!("failed to serialise span snapshot for service broadcast");
    }

    Ok(Json(entry))
}

async fn get_span(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TimelineEntry>> {
    let entry = state
        .repository
        .get_span(id)
        .await?
        .ok_or_else(|| AppError::not_found("span not found"))?;

    Ok(Json(entry))
}

async fn list_spans(
    State(state): State<AppState>,
    Query(query): Query<TimelineQuery>,
) -> AppResult<Json<Vec<TimelineEntry>>> {
    let entries = state.repository.list_spans(&query).await?;
    Ok(Json(entries))
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(err) = handle_socket(socket, state).await {
            warn!(?err, "timeline websocket closed with error");
        }
    })
}

async fn service_ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(err) = handle_service_socket(socket, state).await {
            warn!(?err, "timeline service websocket closed with error");
        }
    })
}

async fn handle_socket(socket: WebSocket, state: AppState) -> AppResult<()> {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.subscribe();

    // Drain incoming messages so the socket stays healthy.
    tokio::spawn(async move {
        while let Some(result) = receiver.next().await {
            if let Err(err) = result {
                error!(?err, "error receiving websocket payload");
                break;
            }
        }
    });

    let ready = serde_json::json!({ "type": "ready" });
    sender
        .send(Message::Text(ready.to_string()))
        .await
        .map_err(|err| AppError::internal(format!("failed to send ready message: {err}")))?;

    while let Ok(entry) = rx.recv().await {
        match serde_json::to_string(&entry) {
            Ok(serialized) => {
                if let Err(err) = sender.send(Message::Text(serialized)).await {
                    return Err(AppError::internal(format!("failed to push span: {err}")));
                }
            }
            Err(err) => {
                warn!(?err, "failed to encode timeline entry");
            }
        }
    }

    Ok(())
}

async fn handle_service_socket(socket: WebSocket, state: AppState) -> AppResult<()> {
    let (mut sender, mut receiver) = socket.split();
    let (peer_id, mut outbound) = state.service_bus.register();
    let service_bus = state.service_bus.clone();

    let hello = ServiceMessage::ServiceHello {
        sender: "logline-timeline".into(),
        capabilities: vec!["timeline_stream".into(), "span_broadcast".into()],
    };
    let hello_message = WebSocketEnvelope::from_service_message(&hello)
        .and_then(|envelope| envelope.to_message())
        .map_err(|err| AppError::internal(format!("failed to encode hello message: {err}")))?;

    sender
        .send(hello_message)
        .await
        .map_err(|err| AppError::internal(format!("failed to send hello message: {err}")))?;

    loop {
        tokio::select! {
            Some(message) = outbound.recv() => {
                let payload = WebSocketEnvelope::from_service_message(&message)
                    .and_then(|envelope| envelope.to_message())
                    .map_err(|err| AppError::internal(format!("failed to encode outbound message: {err}")))?;

                if let Err(err) = sender.send(payload).await {
                    service_bus.unregister(peer_id);
                    return Err(AppError::internal(format!("failed to deliver outbound service message: {err}")));
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        handle_service_payload(&service_bus, peer_id, Message::Text(text))?;
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        handle_service_payload(&service_bus, peer_id, Message::Binary(bytes))?;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if let Err(err) = sender.send(Message::Pong(payload)).await {
                            service_bus.unregister(peer_id);
                            return Err(AppError::internal(format!("failed to respond to ping: {err}")));
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(err)) => {
                        service_bus.unregister(peer_id);
                        return Err(AppError::internal(format!("service websocket error: {err}")));
                    }
                    None => break,
                }
            }
        }
    }

    service_bus.unregister(peer_id);
    Ok(())
}

fn handle_service_payload(
    bus: &ServiceBus,
    peer_id: Uuid,
    message: Message,
) -> Result<(), AppError> {
    let envelope = WebSocketEnvelope::from_message(message)
        .map_err(|err| AppError::internal(format!("invalid service payload: {err}")))?;
    let service_message = envelope
        .into_service_message()
        .map_err(|err| AppError::internal(format!("failed to decode service message: {err}")))?;

    match service_message {
        ServiceMessage::HealthCheckPing => {
            if !bus.send_to(&peer_id, ServiceMessage::HealthCheckPong) {
                warn!(%peer_id, "failed to respond to health check ping");
            }
        }
        ServiceMessage::HealthCheckPong => {
            debug!(%peer_id, "received health check pong");
        }
        ServiceMessage::ServiceHello {
            sender,
            capabilities,
        } => {
            info!(%peer_id, %sender, ?capabilities, "service peer connected");
        }
        ServiceMessage::ConnectionLost { peer } => {
            debug!(%peer_id, %peer, "received connection lost notification");
        }
        other => {
            info!(%peer_id, message = ?other, "received service message");
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct CreateSpanRequest {
    #[serde(default)]
    id: Option<Uuid>,
    #[serde(default)]
    timestamp: Option<chrono::DateTime<Utc>>,
    logline_id: String,
    title: String,
    #[serde(default)]
    status: Option<SpanStatus>,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    contract_id: Option<String>,
    #[serde(default)]
    workflow_id: Option<String>,
    #[serde(default)]
    flow_id: Option<String>,
    #[serde(default)]
    caused_by: Option<Uuid>,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    verification_status: Option<String>,
    #[serde(default)]
    delta_s: Option<f64>,
    #[serde(default)]
    replay_count: Option<u32>,
    #[serde(default)]
    replay_from: Option<Uuid>,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    organization_id: Option<Uuid>,
    #[serde(default)]
    user_id: Option<Uuid>,
    #[serde(default)]
    span_type: Option<SpanType>,
    #[serde(default)]
    visibility: Option<Visibility>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    processed: Option<bool>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    related_spans: Option<Vec<String>>,
}

impl CreateSpanRequest {
    fn into_span(self) -> Span {
        Span {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            logline_id: self.logline_id,
            title: self.title,
            status: self.status.unwrap_or(SpanStatus::Executed),
            data: self.data,
            contract_id: self.contract_id,
            workflow_id: self.workflow_id,
            flow_id: self.flow_id,
            caused_by: self.caused_by,
            signature: self.signature,
            verification_status: self.verification_status,
            delta_s: self.delta_s,
            replay_count: self.replay_count,
            replay_from: self.replay_from,
            tenant_id: self.tenant_id,
            organization_id: self.organization_id,
            user_id: self.user_id,
            span_type: self.span_type,
            visibility: self.visibility,
            metadata: self.metadata,
            processed: self.processed.unwrap_or(false),
            tags: self.tags.unwrap_or_default(),
            related_spans: self.related_spans.unwrap_or_default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ServerError {
    #[error("failed to bind timeline service: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid bind address: {0}")]
    Addr(#[from] std::net::AddrParseError),
    #[error("configuration error: {0}")]
    Config(#[from] LogLineError),
    #[error("http server error: {0}")]
    Server(#[from] HyperError),
}

#[derive(Debug, Clone)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn bad_request<M: Into<String>>(message: M) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found<M: Into<String>>(message: M) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal<M: Into<String>>(message: M) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({ "error": self.message }));
        (self.status, body).into_response()
    }
}

impl From<LogLineError> for AppError {
    fn from(err: LogLineError) -> Self {
        match err {
            LogLineError::InvalidSpanId(message) => AppError::bad_request(message),
            LogLineError::SpanNotFound(message) => AppError::not_found(message),
            other => AppError::internal(other.to_string()),
        }
    }
}
