use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::motor::types::{Span, SpanStatus, SpanType, Visibility};
use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Timeline {
    pool: PgPool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineQuery {
    pub logline_id: Option<String>,
    pub contract_id: Option<String>,
    pub workflow_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    // Multi-tenant filters
    pub tenant_id: Option<String>,
    pub organization_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub span_type: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub logline_id: String,
    pub author: String,
    pub title: String,
    pub payload: serde_json::Value,
    pub contract_id: Option<String>,
    pub workflow_id: Option<String>,
    pub flow_id: Option<String>,
    pub caused_by: Option<Uuid>,
    pub signature: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    // Multi-tenant fields
    pub tenant_id: Option<String>,
    pub organization_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub span_type: Option<String>,
    pub visibility: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub organization_name: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Timeline {
    /// Cria nova instância da Timeline com conexão PostgreSQL
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Timeline { pool })
    }
    
    /// Cria um novo span específico para um tenant
    pub async fn create_tenant_span(
        &self,
        tenant_id: &str,
        user_id: Uuid,
        span: &mut Span,
        id_with_keys: &LogLineIDWithKeys
    ) -> Result<Uuid, sqlx::Error> {
        // Set tenant context in the span
        span.tenant_id = Some(tenant_id.to_string());
        span.user_id = Some(user_id);
        
        // Sign and append the span
        self.append_signed_span(span, id_with_keys).await
    }
    
    /// Get timeline for a specific tenant with access control
    pub async fn get_tenant_timeline(
        &self,
        tenant_id: &str,
        user_id: Option<Uuid>,
        limit: Option<i64>,
        offset: Option<i64>
    ) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        // Call the PostgreSQL function
        let rows = sqlx::query(
            r#"
            SELECT * FROM get_tenant_timeline($1, $2, $3, $4)
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(limit.unwrap_or(100) as i32)
        .bind(offset.unwrap_or(0) as i32)
        .fetch_all(&self.pool)
        .await?;
        
        // Map rows to TimelineEntry
        let entries: Vec<TimelineEntry> = rows.into_iter()
            .map(|row| {
                TimelineEntry {
                    id: row.get("id"),
                    timestamp: row.get("timestamp"),
                    logline_id: row.get("logline_id"),
                    author: row.get("author"),
                    title: row.get("title"),
                    payload: serde_json::Value::Null, // Not included in the function result
                    contract_id: None, // Not included in the function result
                    workflow_id: None, // Not included in the function result
                    flow_id: None, // Not included in the function result
                    caused_by: None, // Not included in the function result
                    signature: None, // Not included in the function result
                    status: "executed".to_string(), // Default status
                    created_at: Utc::now(), // Default as not included in the function result
                    tenant_id: Some(tenant_id.to_string()),
                    organization_id: None, // Not included in the function result
                    user_id: None, // Not included in the function result
                    span_type: row.get("span_type"),
                    visibility: row.get("visibility"),
                    metadata: None, // Not included in the function result
                    organization_name: row.get("organization_name"),
                    updated_at: None, // Not included in the function result
                }
            })
            .collect();
            
        Ok(entries)
    }
    
    /// Get statistics for a specific tenant
    pub async fn get_tenant_stats(
        &self, 
        tenant_id: &str
    ) -> Result<serde_json::Value, sqlx::Error> {
        let row = sqlx::query("SELECT * FROM get_tenant_stats($1)")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await?;
            
        // Build stats object
        let stats = serde_json::json!({
            "total_spans": row.get::<i64, _>("total_spans"),
            "active_users": row.get::<i64, _>("active_users"),
            "spans_today": row.get::<i64, _>("spans_today"),
            "spans_this_week": row.get::<i64, _>("spans_this_week"),
            "most_active_author": row.get::<Option<String>, _>("most_active_author"),
            "latest_activity": row.get::<Option<DateTime<Utc>>, _>("latest_activity"),
        });
        
        Ok(stats)
    }
    
    /// Anexa um span à timeline (append-only)
    pub async fn append_span(&self, span: &Span) -> Result<Uuid, sqlx::Error> {
        let status_str = match span.status {
            SpanStatus::Executed => "executed",
            SpanStatus::Simulated => "simulated", 
            SpanStatus::Reverted => "reverted",
            SpanStatus::Ghost => "ghost",
        };
        
        // Convert span_type to string if present
        let span_type_str = match &span.span_type {
            Some(SpanType::User) => Some("user".to_string()),
            Some(SpanType::System) => Some("system".to_string()),
            Some(SpanType::Organization) => Some("organization".to_string()),
            Some(SpanType::Ghost) => Some("ghost".to_string()),
            None => None,
        };
        
        // Convert visibility to string if present
        let visibility_str = match &span.visibility {
            Some(Visibility::Private) => Some("private".to_string()),
            Some(Visibility::Organization) => Some("organization".to_string()),
            Some(Visibility::Public) => Some("public".to_string()),
            None => None,
        };
        
        let result = sqlx::query(
            r#"
            INSERT INTO timeline_spans (
                id, timestamp, logline_id, author, title, payload, 
                contract_id, workflow_id, flow_id, caused_by, signature, status,
                tenant_id, organization_id, user_id, span_type, visibility, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#
        )
        .bind(&span.id)
        .bind(&span.timestamp)
        .bind(&span.logline_id)
        .bind(&span.logline_id) // author same as logline_id for now
        .bind(&span.title)
        .bind(&span.payload)
        .bind(&span.contract_id)
        .bind(&span.workflow_id)
        .bind(&span.flow_id)
        .bind(&span.caused_by)
        .bind(&span.signature)
        .bind(status_str)
        .bind(&span.tenant_id)
        .bind(&span.organization_id)
        .bind(&span.user_id)
        .bind(&span_type_str)
        .bind(&visibility_str)
        .bind(&span.metadata)
        .execute(&self.pool)
        .await?;
        
        println!("✅ Span anexado à timeline: {}", span.id);
        Ok(span.id)
    }
    
    /// Assina um span antes de anexar à timeline
    pub async fn append_signed_span(&self, span: &mut Span, id_with_keys: &LogLineIDWithKeys) -> Result<Uuid, sqlx::Error> {
        // Serializa o span para assinatura
        let span_data = serde_json::to_string(span).unwrap();
        let signature = id_with_keys.id.sign(&id_with_keys.signing_key, span_data.as_bytes());
        
        // Adiciona assinatura ao span
        span.signature = Some(hex::encode(signature.to_bytes()));
        
        // Anexa à timeline
        self.append_span(span).await
    }
    
    /// Lista spans com filtros opcionais
    pub async fn list_spans(&self, query: &TimelineQuery) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        let mut sql_parts = vec![
            "SELECT ts.id, ts.timestamp, ts.logline_id, ts.author, ts.title, ts.payload, 
             ts.contract_id, ts.workflow_id, ts.flow_id, ts.caused_by, ts.signature, ts.status, ts.created_at,
             ts.tenant_id, ts.organization_id, ts.user_id, ts.span_type, ts.visibility, ts.metadata, ts.updated_at,
             o.name as organization_name
             FROM timeline_spans ts
             LEFT JOIN organizations o ON ts.organization_id = o.id
             WHERE 1=1".to_string()
        ];
        
        let mut params: Vec<String> = vec![];
        
        // Add tenant_id filter if present
        if let Some(ref tenant_id) = query.tenant_id {
            sql_parts.push(format!(" AND ts.tenant_id = '{}'", tenant_id));
        }
        
        // Add organization_id filter if present
        if let Some(ref org_id) = query.organization_id {
            sql_parts.push(format!(" AND ts.organization_id = '{}'", org_id));
        }
        
        // Add user_id filter if present
        if let Some(ref user_id) = query.user_id {
            sql_parts.push(format!(" AND ts.user_id = '{}'", user_id));
        }
        
        // Add span_type filter if present
        if let Some(ref span_type) = query.span_type {
            sql_parts.push(format!(" AND ts.span_type = '{}'", span_type));
        }
        
        // Add visibility filter if present
        if let Some(ref visibility) = query.visibility {
            sql_parts.push(format!(" AND ts.visibility = '{}'", visibility));
        }
        
        // Add logline_id filter if present
        if let Some(ref logline_id) = query.logline_id {
            sql_parts.push(format!(" AND ts.logline_id = '{}'", logline_id));
        }
        
        // Add contract_id filter if present
        if let Some(ref contract_id) = query.contract_id {
            sql_parts.push(format!(" AND ts.contract_id = '{}'", contract_id));
        }
        
        // Add workflow_id filter if present
        if let Some(ref workflow_id) = query.workflow_id {
            sql_parts.push(format!(" AND ts.workflow_id = '{}'", workflow_id));
        }
        
        // Add order by and limit
        sql_parts.push(format!(" ORDER BY ts.timestamp DESC LIMIT {}", query.limit.unwrap_or(50)));
        
        // Add offset if present
        if let Some(offset) = query.offset {
            sql_parts.push(format!(" OFFSET {}", offset));
        }
        
        // Combine SQL parts
        let sql = sql_parts.join("");
        
        // Execute query
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await?;
        
        let entries: Vec<TimelineEntry> = rows.into_iter().map(|row| {
            TimelineEntry {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                logline_id: row.get("logline_id"),
                author: row.get("author"),
                title: row.get("title"),
                payload: row.get("payload"),
                contract_id: row.get("contract_id"),
                workflow_id: row.get("workflow_id"),
                flow_id: row.get("flow_id"),
                caused_by: row.get("caused_by"),
                signature: row.get("signature"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                // Multi-tenant fields
                tenant_id: row.get("tenant_id"),
                organization_id: row.get("organization_id"),
                user_id: row.get("user_id"),
                span_type: row.get("span_type"),
                visibility: row.get("visibility"),
                metadata: row.get("metadata"),
                organization_name: row.get("organization_name"),
                updated_at: row.get("updated_at"),
            }
        }).collect();
        
        Ok(entries)
    }
    
    /// Obtém um span específico por ID
    pub async fn get_span(&self, span_id: Uuid) -> Result<Option<TimelineEntry>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT ts.id, ts.timestamp, ts.logline_id, ts.author, ts.title, ts.payload, 
             ts.contract_id, ts.workflow_id, ts.flow_id, ts.caused_by, ts.signature, ts.status, ts.created_at,
             ts.tenant_id, ts.organization_id, ts.user_id, ts.span_type, ts.visibility, ts.metadata, ts.updated_at,
             o.name as organization_name
             FROM timeline_spans ts
             LEFT JOIN organizations o ON ts.organization_id = o.id
             WHERE ts.id = $1"
        )
        .bind(span_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|r| TimelineEntry {
            id: r.get("id"),
            timestamp: r.get("timestamp"),
            logline_id: r.get("logline_id"),
            author: r.get("author"),
            title: r.get("title"),
            payload: r.get("payload"),
            contract_id: r.get("contract_id"),
            workflow_id: r.get("workflow_id"),
            flow_id: r.get("flow_id"),
            caused_by: r.get("caused_by"),
            signature: r.get("signature"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            // Multi-tenant fields
            tenant_id: r.get("tenant_id"),
            organization_id: r.get("organization_id"),
            user_id: r.get("user_id"),
            span_type: r.get("span_type"),
            visibility: r.get("visibility"),
            metadata: r.get("metadata"),
            organization_name: r.get("organization_name"),
            updated_at: r.get("updated_at"),
        }))
    }
    
    /// Verifica integridade da timeline
    pub async fn verify_integrity(&self) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM timeline_spans WHERE signature IS NULL OR signature = ''"
        )
        .fetch_one(&self.pool)
        .await?;
        
        let count: i64 = row.get("count");
        Ok(count == 0)
    }
    
    /// Busca spans por texto
    pub async fn search_spans(&self, search_term: &str, tenant_id: Option<&str>, limit: Option<i64>) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        let mut sql_parts = vec![
            "SELECT ts.id, ts.timestamp, ts.logline_id, ts.author, ts.title, ts.payload, 
             ts.contract_id, ts.workflow_id, ts.flow_id, ts.caused_by, ts.signature, ts.status, ts.created_at,
             ts.tenant_id, ts.organization_id, ts.user_id, ts.span_type, ts.visibility, ts.metadata, ts.updated_at,
             o.name as organization_name
             FROM timeline_spans ts
             LEFT JOIN organizations o ON ts.organization_id = o.id
             WHERE to_tsvector('portuguese', ts.title || ' ' || ts.payload::text) @@ plainto_tsquery('portuguese', '".to_string(),
            search_term.to_string(),
            "')".to_string(),
        ];
        
        // Add tenant_id filter if present
        if let Some(tenant) = tenant_id {
            sql_parts.push(format!(" AND ts.tenant_id = '{}'", tenant));
        }
        
        // Add order by and limit
        sql_parts.push(format!(" ORDER BY ts.timestamp DESC LIMIT {}", limit.unwrap_or(20)));
        
        // Combine SQL parts
        let sql = sql_parts.join("");
        
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await?;
        
        let entries: Vec<TimelineEntry> = rows.into_iter().map(|row| {
            TimelineEntry {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                logline_id: row.get("logline_id"),
                author: row.get("author"),
                title: row.get("title"),
                payload: row.get("payload"),
                contract_id: row.get("contract_id"),
                workflow_id: row.get("workflow_id"),
                flow_id: row.get("flow_id"),
                caused_by: row.get("caused_by"),
                signature: row.get("signature"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                // Multi-tenant fields
                tenant_id: row.get("tenant_id"),
                organization_id: row.get("organization_id"),
                user_id: row.get("user_id"),
                span_type: row.get("span_type"),
                visibility: row.get("visibility"),
                metadata: row.get("metadata"),
                organization_name: row.get("organization_name"),
                updated_at: row.get("updated_at"),
            }
        }).collect();
        
        Ok(entries)
    }
}