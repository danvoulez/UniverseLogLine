use chrono::{DateTime, Utc};
use logline_core::config::CoreConfig;
use logline_core::db::DatabasePool;
use logline_core::errors::{LogLineError, Result};
use logline_protocol::timeline::{Span, SpanStatus, TimelineEntry, TimelineQuery};
use serde_json::Value;
use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

/// Database-backed repository for timeline spans.
#[derive(Clone)]
pub struct TimelineRepository {
    pool: DatabasePool,
}

impl TimelineRepository {
    /// Connects to the database using the supplied configuration and ensures migrations ran.
    pub async fn from_config(config: &CoreConfig) -> Result<Self> {
        let pool = DatabasePool::connect(config).await?;
        Self::from_pool(pool).await
    }

    /// Builds the repository from an existing database pool.
    pub async fn from_pool(pool: DatabasePool) -> Result<Self> {
        sqlx::migrate!("../timeline/migrations")
            .run(pool.inner())
            .await
            .map_err(|err| LogLineError::TimelineError(err.to_string()))?;
        Ok(Self { pool })
    }

    /// Inserts a new span into the timeline and returns the stored representation.
    pub async fn create_span(&self, span: Span) -> Result<TimelineEntry> {
        let row = sqlx::query_as::<_, TimelineSpanRow>(
            r#"
            INSERT INTO timeline_spans (
                id, timestamp, logline_id, author, title, payload,
                contract_id, workflow_id, flow_id, caused_by, signature,
                status, verification_status, delta_s, replay_count, replay_from
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16
            )
            RETURNING
                id, timestamp, logline_id, author, title, payload,
                contract_id, workflow_id, flow_id, caused_by, signature,
                status, verification_status, delta_s, replay_count, replay_from,
                created_at, updated_at
            "#,
        )
        .bind(span.id)
        .bind(span.timestamp)
        .bind(&span.logline_id)
        .bind(&span.author)
        .bind(&span.title)
        .bind(
            span.data
                .clone()
                .unwrap_or_else(|| Value::Object(Default::default())),
        )
        .bind(&span.contract_id)
        .bind(&span.workflow_id)
        .bind(&span.flow_id)
        .bind(&span.caused_by)
        .bind(
            span.signature
                .clone()
                .unwrap_or_else(|| "unsigned".to_string()),
        )
        .bind(Self::status_to_str(span.status))
        .bind(
            span.verification_status
                .clone()
                .unwrap_or_else(|| "verified".to_string()),
        )
        .bind(span.delta_s.unwrap_or(0.0))
        .bind(span.replay_count.map(|value| value as i32).unwrap_or(0))
        .bind(&span.replay_from)
        .fetch_one(self.pool.inner())
        .await?;

        Ok(row.into())
    }

    /// Fetches a span by its identifier.
    pub async fn get_span(&self, id: Uuid) -> Result<Option<TimelineEntry>> {
        let row = sqlx::query_as::<_, TimelineSpanRow>(
            r#"
            SELECT
                id, timestamp, logline_id, author, title, payload,
                contract_id, workflow_id, flow_id, caused_by, signature,
                status, verification_status, delta_s, replay_count, replay_from,
                created_at, updated_at
            FROM timeline_spans
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.inner())
        .await?;

        Ok(row.map(Into::into))
    }

    /// Lists spans based on the provided query filters.
    pub async fn list_spans(&self, query: &TimelineQuery) -> Result<Vec<TimelineEntry>> {
        let mut builder = QueryBuilder::new(
            "SELECT id, timestamp, logline_id, author, title, payload, \
             contract_id, workflow_id, flow_id, caused_by, signature, \
             status, verification_status, delta_s, replay_count, replay_from, \
             created_at, updated_at FROM timeline_spans WHERE 1=1",
        );

        if let Some(logline_id) = &query.logline_id {
            builder.push(" AND logline_id = ");
            builder.push_bind(logline_id);
        }

        if let Some(contract_id) = &query.contract_id {
            builder.push(" AND contract_id = ");
            builder.push_bind(contract_id);
        }

        if let Some(workflow_id) = &query.workflow_id {
            builder.push(" AND workflow_id = ");
            builder.push_bind(workflow_id);
        }

        builder.push(" ORDER BY timestamp DESC");

        if let Some(limit) = query.limit {
            builder.push(" LIMIT ");
            builder.push_bind(limit);
        }

        if let Some(offset) = query.offset {
            builder.push(" OFFSET ");
            builder.push_bind(offset);
        }

        let rows = builder
            .build_query_as::<TimelineSpanRow>()
            .fetch_all(self.pool.inner())
            .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    fn status_to_str(status: SpanStatus) -> &'static str {
        match status {
            SpanStatus::Executed => "executed",
            SpanStatus::Simulated => "simulated",
            SpanStatus::Reverted => "reverted",
            SpanStatus::Ghost => "ghost",
        }
    }
}

#[derive(FromRow)]
struct TimelineSpanRow {
    id: Uuid,
    timestamp: DateTime<Utc>,
    logline_id: String,
    author: String,
    title: String,
    payload: Value,
    contract_id: Option<String>,
    workflow_id: Option<String>,
    flow_id: Option<String>,
    caused_by: Option<Uuid>,
    signature: String,
    status: String,
    verification_status: String,
    delta_s: Option<f64>,
    replay_count: Option<i32>,
    #[allow(dead_code)]
    #[sqlx(rename = "replay_from")]
    _replay_from: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<TimelineSpanRow> for TimelineEntry {
    fn from(row: TimelineSpanRow) -> Self {
        TimelineEntry {
            id: row.id,
            timestamp: row.timestamp,
            logline_id: row.logline_id,
            author: row.author,
            title: row.title,
            payload: row.payload,
            contract_id: row.contract_id,
            workflow_id: row.workflow_id,
            flow_id: row.flow_id,
            caused_by: row.caused_by,
            signature: Some(row.signature),
            status: row.status,
            created_at: row.created_at,
            tenant_id: None,
            organization_id: None,
            user_id: None,
            span_type: None,
            visibility: None,
            metadata: None,
            organization_name: None,
            updated_at: Some(row.updated_at),
            delta_s: row.delta_s,
            replay_count: row.replay_count.map(|value| value as u32),
            verification_status: Some(row.verification_status),
        }
    }
}
