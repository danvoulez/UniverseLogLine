mod repository;

use std::net::SocketAddr;

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
use logline_protocol::timeline::{
    Span, SpanStatus, SpanType, TimelineEntry, TimelineQuery, Visibility,
};
use repository::TimelineRepository;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
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

    let state = AppState {
        repository,
        broadcaster: tx,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/spans", get(list_spans).post(create_span))
        .route("/v1/spans/:id", get(get_span))
        .route("/ws", get(ws_upgrade))
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
}

impl AppState {
    fn subscribe(&self) -> broadcast::Receiver<TimelineEntry> {
        self.broadcaster.subscribe()
    }
}

type AppResult<T> = Result<T, AppError>;

async fn create_span(
    State(state): State<AppState>,
    Json(payload): Json<CreateSpanRequest>,
) -> AppResult<Json<TimelineEntry>> {
    let span = payload.into_span();
    let entry = state.repository.create_span(span).await?;

    if let Err(err) = state.broadcaster.send(entry.clone()) {
        warn!(?err, "failed to broadcast new span");
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
