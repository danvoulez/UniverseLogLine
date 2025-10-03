use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use logline_protocol::timeline::Span;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::info;

use crate::{EnforcementOutcome, Rule, RuleEngine, RuleStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDocument {
    pub tenant_id: String,
    pub rule: Rule,
    #[serde(default)]
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResponse {
    pub version: u32,
    pub rule: Rule,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: Option<String>,
}

impl From<crate::RuleHistoryEntry> for RuleResponse {
    fn from(value: crate::RuleHistoryEntry) -> Self {
        Self {
            version: value.version,
            rule: value.rule,
            created_at: value.created_at,
            updated_by: value.updated_by,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRequest {
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResponse {
    pub decision: String,
    pub applied_rules: Vec<String>,
    pub notes: Vec<String>,
    pub tags: Vec<String>,
}

impl From<EnforcementOutcome> for EvaluationResponse {
    fn from(value: EnforcementOutcome) -> Self {
        let decision = match value.decision {
            crate::Decision::Allow => "allow".to_string(),
            crate::Decision::Reject { .. } => "reject".to_string(),
            crate::Decision::Simulate { .. } => "simulate".to_string(),
        };

        Self {
            decision,
            applied_rules: value.applied_rules,
            notes: value.notes,
            tags: value.added_tags,
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

#[derive(Clone)]
struct RuleServiceState {
    store: RuleStore,
}

/// Configuration for the rule API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleServiceConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
}

fn default_bind_address() -> String {
    "0.0.0.0:8081".to_string()
}

impl Default for RuleServiceConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
        }
    }
}

/// Helper used by services to compose the REST API router.
#[derive(Clone)]
pub struct RuleApiBuilder {
    state: RuleServiceState,
}

impl RuleApiBuilder {
    pub fn new(store: RuleStore) -> Self {
        Self {
            state: RuleServiceState { store },
        }
    }

    pub fn into_router(self) -> Router {
        Router::new()
            .route("/health", get(health))
            .route("/tenants", get(list_tenants))
            .route("/tenants/:tenant/rules", get(list_rules).post(upsert_rule))
            .route(
                "/tenants/:tenant/rules/:rule_id",
                get(get_rule).put(disable_rule),
            )
            .route("/tenants/:tenant/evaluate", post(evaluate_span))
            .with_state(self.state)
    }

    /// Spawns an HTTP server binding to the configured address.
    pub async fn serve(self, config: RuleServiceConfig) -> anyhow::Result<oneshot::Sender<()>> {
        let (tx, rx) = oneshot::channel();
        let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
        let state = self.state.clone();

        tokio::spawn(async move {
            info!(address = %config.bind_address, "starting rule service");
            let app = RuleApiBuilder { state }.into_router();
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await
                .ok();
        });

        Ok(tx)
    }
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn list_tenants(State(state): State<RuleServiceState>) -> impl IntoResponse {
    Json(state.store.tenants())
}

async fn list_rules(
    State(state): State<RuleServiceState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let response: Vec<RuleResponse> = state
        .store
        .list_rules(&tenant)
        .into_iter()
        .map(RuleResponse::from)
        .collect();
    Json(response)
}

async fn get_rule(
    State(state): State<RuleServiceState>,
    Path((tenant, rule_id)): Path<(String, String)>,
) -> Result<Json<RuleResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .store
        .latest_rule(&tenant, &rule_id)
        .map(RuleResponse::from)
        .map(Json)
        .ok_or_else(|| rule_not_found(&rule_id))
}

#[derive(Debug, Deserialize)]
struct DisableRequest {
    #[serde(default)]
    updated_by: Option<String>,
}

async fn disable_rule(
    State(state): State<RuleServiceState>,
    Path((tenant, rule_id)): Path<(String, String)>,
    Json(payload): Json<DisableRequest>,
) -> Result<Json<RuleResponse>, (StatusCode, Json<ErrorResponse>)> {
    let entry = state
        .store
        .disable_rule(&tenant, &rule_id, payload.updated_by)
        .map(RuleResponse::from)
        .map(Json)
        .map_err(|_| rule_not_found(&rule_id))?;
    Ok(entry)
}

async fn upsert_rule(
    State(state): State<RuleServiceState>,
    Path(tenant): Path<String>,
    Json(payload): Json<RuleDocument>,
) -> Result<Json<RuleResponse>, (StatusCode, String)> {
    if !payload.tenant_id.is_empty() && payload.tenant_id != tenant {
        return Err((
            StatusCode::BAD_REQUEST,
            "tenant identifier mismatch".to_string(),
        ));
    }

    let entry = state
        .store
        .put_rule(&tenant, payload.rule, payload.updated_by)
        .into();
    Ok(Json::<RuleResponse>(entry))
}

async fn evaluate_span(
    State(state): State<RuleServiceState>,
    Path(tenant): Path<String>,
    Json(payload): Json<EvaluationRequest>,
) -> impl IntoResponse {
    let engine: RuleEngine = state.store.engine_for(&tenant);
    let mut span = payload.span;
    let outcome = engine.apply(&mut span);
    Json(EvaluationResponse::from(outcome))
}

fn rule_not_found(id: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            code: "not_found".into(),
            message: format!("rule {} not found", id),
        }),
    )
}
