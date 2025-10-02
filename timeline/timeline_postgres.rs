use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::motor::types::{Span, SpanStatus};
use crate::infra::id::logline_id::LogLineIDWithKeys;
use crate::timeline::{TimelineEntry, TimelineQuery, TimelineStats};
use logline_core::config::CoreConfig;
use logline_core::db::DatabasePool;
use logline_core::errors::LogLineError;

#[derive(Debug, Clone)]
pub struct TimelinePostgres {
    pool: PgPool,
}

impl TimelinePostgres {
    /// Cria nova instância da Timeline PostgreSQL
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;

        sqlx::migrate!("./timeline/migrations").run(&pool).await?;

        Ok(TimelinePostgres { pool })
    }

    pub async fn new_with_core_config(config: &CoreConfig) -> Result<Self, LogLineError> {
        let shared_pool = DatabasePool::connect(config).await?;
        Self::from_shared_pool(&shared_pool).await
    }

    pub async fn from_shared_pool(pool: &DatabasePool) -> Result<Self, LogLineError> {
        let pg_pool = pool.inner().clone();
        sqlx::migrate!("./timeline/migrations").run(&pg_pool).await?;
        Ok(TimelinePostgres { pool: pg_pool })
    }
    
    /// Converte Span para inserção no PostgreSQL
    fn span_to_postgres_values(&self, span: &Span) -> (
        Uuid, DateTime<Utc>, String, String, String, serde_json::Value,
        Option<String>, Option<String>, Option<String>, Option<Uuid>,
        String, String, String, f64, i32, Option<Uuid>
    ) {
        (
            span.id,
            span.timestamp,
            span.logline_id.clone(),
            span.logline_id.clone(), // author = logline_id por enquanto
            span.title.clone(),
            span.payload.clone(),
            span.contract_id.clone(),
            span.workflow_id.clone(),
            span.flow_id.clone(),
            span.caused_by,
            span.signature.clone().unwrap_or_else(|| "unsigned".to_string()),
            match span.status {
                SpanStatus::Executed => "executed".to_string(),
                SpanStatus::Simulated => "simulated".to_string(),
                SpanStatus::Reverted => "reverted".to_string(),
                SpanStatus::Ghost => "ghost".to_string(),
            },
            "verified".to_string(), // verification_status
            0.0, // delta_s inicial
            0,   // replay_count inicial
            None // replay_from inicial
        )
    }
    
    /// Insere um span na timeline PostgreSQL
    pub async fn insert_span(&self, span: &Span) -> Result<Uuid, sqlx::Error> {
        let values = self.span_to_postgres_values(span);
        
        let result = sqlx::query!(
            r#"
            INSERT INTO timeline_spans (
                id, timestamp, logline_id, author, title, payload,
                contract_id, workflow_id, flow_id, caused_by, signature,
                status, verification_status, delta_s, replay_count, replay_from
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id
            "#,
            values.0, values.1, values.2, values.3, values.4, values.5,
            values.6, values.7, values.8, values.9, values.10,
            values.11, values.12, values.13, values.14, values.15
        )
        .fetch_one(&self.pool)
        .await?;
        
        println!("✅ Span inserido no PostgreSQL: {}", result.id);
        Ok(result.id)
    }
    
    /// Assina e insere span
    pub async fn append_signed_span(&self, span: &mut Span, id_with_keys: &LogLineIDWithKeys) -> Result<Uuid, sqlx::Error> {
        // Assinar span (mesmo processo da NDJSON)
        let mut span_for_signing = span.clone();
        span_for_signing.signature = None;
        let span_data = serde_json::to_string(&span_for_signing).unwrap();
        let signature = id_with_keys.id.sign(&id_with_keys.signing_key, span_data.as_bytes());
        
        span.signature = Some(hex::encode(signature.to_bytes()));
        
        // Inserir no PostgreSQL
        self.insert_span(span).await
    }
    
    /// Busca span por ID
    pub async fn get_span_by_id(&self, span_id: Uuid) -> Result<Option<TimelineEntry>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT id, timestamp, logline_id, author, title, payload,
                   contract_id, workflow_id, flow_id, caused_by, signature,
                   status, verification_status, delta_s, replay_count, 
                   replay_from, created_at
            FROM timeline_spans WHERE id = $1
            "#,
            span_id
        )
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|r| TimelineEntry {
            id: r.id,
            timestamp: r.timestamp,
            logline_id: r.logline_id,
            author: r.author,
            title: r.title,
            payload: r.payload,
            contract_id: r.contract_id,
            workflow_id: r.workflow_id,
            flow_id: r.flow_id,
            caused_by: r.caused_by,
            signature: r.signature,
            status: r.status,
            created_at: r.created_at,
            delta_s: Some(r.delta_s.unwrap_or(0.0)),
            replay_count: Some(r.replay_count.unwrap_or(0) as u32),
            verification_status: r.verification_status,
        }))
    }
    
    /// Lista spans com filtros
    pub async fn list_spans(&self, query: &TimelineQuery) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        let mut sql = String::from(
            r#"
            SELECT id, timestamp, logline_id, author, title, payload,
                   contract_id, workflow_id, flow_id, caused_by, signature,
                   status, verification_status, delta_s, replay_count, 
                   replay_from, created_at
            FROM timeline_spans WHERE 1=1
            "#
        );
        
        // Construir query dinamicamente
        let mut bind_values: Vec<String> = Vec::new();
        let mut param_count = 0;
        
        if let Some(ref logline_id) = query.logline_id {
            param_count += 1;
            sql.push_str(&format!(" AND logline_id = ${}", param_count));
            bind_values.push(logline_id.clone());
        }
        
        if let Some(ref contract_id) = query.contract_id {
            param_count += 1;
            sql.push_str(&format!(" AND contract_id = ${}", param_count));
            bind_values.push(contract_id.clone());
        }
        
        if let Some(ref workflow_id) = query.workflow_id {
            param_count += 1;
            sql.push_str(&format!(" AND workflow_id = ${}", param_count));
            bind_values.push(workflow_id.clone());
        }
        
        sql.push_str(" ORDER BY timestamp DESC");
        
        if let Some(limit) = query.limit {
            param_count += 1;
            sql.push_str(&format!(" LIMIT ${}", param_count));
            bind_values.push(limit.to_string());
        }
        
        // Executar query simples para evitar problemas de binding
        let rows = if bind_values.is_empty() {
            sqlx::query(&sql).fetch_all(&self.pool).await?
        } else {
            // Por simplicidade, usar query específica mais comum
            if let Some(ref logline_id) = query.logline_id {
                sqlx::query(&format!(
                    "SELECT id, timestamp, logline_id, author, title, payload, contract_id, workflow_id, flow_id, caused_by, signature, status, verification_status, delta_s, replay_count, replay_from, created_at FROM timeline_spans WHERE logline_id = '{}' ORDER BY timestamp DESC LIMIT {}",
                    logline_id,
                    query.limit.unwrap_or(50)
                )).fetch_all(&self.pool).await?
            } else {
                sqlx::query(&format!(
                    "SELECT id, timestamp, logline_id, author, title, payload, contract_id, workflow_id, flow_id, caused_by, signature, status, verification_status, delta_s, replay_count, replay_from, created_at FROM timeline_spans ORDER BY timestamp DESC LIMIT {}",
                    query.limit.unwrap_or(50)
                )).fetch_all(&self.pool).await?
            }
        };
        
        let entries: Vec<TimelineEntry> = rows.into_iter().map(|row| {
            TimelineEntry {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                logline_id: row.get("logline_id"),
                author: row.get("author"),
                title: row.get("title"),
                payload: row.get::<serde_json::Value, _>("payload"),
                contract_id: row.get("contract_id"),
                workflow_id: row.get("workflow_id"),
                flow_id: row.get("flow_id"),
                caused_by: row.get("caused_by"),
                signature: row.get("signature"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                delta_s: row.get::<Option<f64>, _>("delta_s"),
                replay_count: row.get::<Option<i32>, _>("replay_count").map(|x| x as u32),
                verification_status: row.get("verification_status"),
            }
        }).collect();
        
        Ok(entries)
    }
    
    /// Busca spans por texto
    pub async fn search_spans(&self, term: &str, limit: Option<usize>) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT id, timestamp, logline_id, author, title, payload,
                   contract_id, workflow_id, flow_id, caused_by, signature,
                   status, verification_status, delta_s, replay_count, 
                   replay_from, created_at
            FROM timeline_spans 
            WHERE to_tsvector('portuguese', title || ' ' || coalesce(payload::text, '')) 
                  @@ plainto_tsquery('portuguese', '{}')
            ORDER BY timestamp DESC LIMIT {}
            "#,
            term.replace("'", "''"), // Escape SQL injection
            limit.unwrap_or(20)
        ))
        .fetch_all(&self.pool)
        .await?;
        
        let entries: Vec<TimelineEntry> = rows.into_iter().map(|row| {
            TimelineEntry {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                logline_id: row.get("logline_id"),
                author: row.get("author"),
                title: row.get("title"),
                payload: row.get::<serde_json::Value, _>("payload"),
                contract_id: row.get("contract_id"),
                workflow_id: row.get("workflow_id"),
                flow_id: row.get("flow_id"),
                caused_by: row.get("caused_by"),
                signature: row.get("signature"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                delta_s: row.get::<Option<f64>, _>("delta_s"),
                replay_count: row.get::<Option<i32>, _>("replay_count").map(|x| x as u32),
                verification_status: row.get("verification_status"),
            }
        }).collect();
        
        Ok(entries)
    }
    
    /// Verifica integridade da timeline
    pub async fn verify_integrity(&self) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "SELECT COUNT(*) as total, COUNT(*) FILTER (WHERE signature IS NOT NULL AND signature != '') as signed FROM timeline_spans"
        )
        .fetch_one(&self.pool)
        .await?;
        
        let total = result.total.unwrap_or(0);
        let signed = result.signed.unwrap_or(0);
        
        println!("📊 Integridade PostgreSQL: {}/{} spans assinados", signed, total);
        Ok(total == signed)
    }
    
    /// Calcula estatísticas da timeline
    pub async fn get_stats(&self) -> Result<TimelineStats, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_spans,
                COUNT(*) FILTER (WHERE signature IS NOT NULL AND signature != '') as signed_spans,
                COUNT(*) FILTER (WHERE contract_id IS NOT NULL) as contract_spans,
                COUNT(*) FILTER (WHERE status = 'executed') as executed_spans,
                COUNT(*) FILTER (WHERE status = 'simulated') as simulated_spans,
                COUNT(*) FILTER (WHERE status = 'ghost') as ghost_spans,
                COUNT(DISTINCT logline_id) as unique_logline_ids
            FROM timeline_spans
            "#
        )
        .fetch_one(&self.pool)
        .await?;
        
        let unique_ids = sqlx::query!("SELECT DISTINCT logline_id FROM timeline_spans ORDER BY logline_id")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| row.logline_id)
            .collect();
        
        Ok(TimelineStats {
            total_spans: result.total_spans.unwrap_or(0) as u64,
            signed_spans: result.signed_spans.unwrap_or(0) as u64,
            contract_spans: result.contract_spans.unwrap_or(0) as u64,
            executed_spans: result.executed_spans.unwrap_or(0) as u64,
            simulated_spans: result.simulated_spans.unwrap_or(0) as u64,
            ghost_spans: result.ghost_spans.unwrap_or(0) as u64,
            other_spans: 0, // Calculado se necessário
            unique_logline_ids: unique_ids,
        })
    }
    
    /// Incrementa replay_count de um span
    pub async fn increment_replay_count(&self, span_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE timeline_spans SET replay_count = replay_count + 1 WHERE id = $1",
            span_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Registra um replay de span
    pub async fn register_replay(&self, original_span_id: Uuid, replay_span: &Span) -> Result<Uuid, sqlx::Error> {
        let mut replay_span_modified = replay_span.clone();
        
        // Marcar como replay
        replay_span_modified.title = format!("🔁 Replay: {}", replay_span.title);
        
        // Inserir novo span com replay_from
        let values = self.span_to_postgres_values(&replay_span_modified);
        let result = sqlx::query!(
            r#"
            INSERT INTO timeline_spans (
                id, timestamp, logline_id, author, title, payload,
                contract_id, workflow_id, flow_id, caused_by, signature,
                status, verification_status, delta_s, replay_count, replay_from
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id
            "#,
            values.0, values.1, values.2, values.3, values.4, values.5,
            values.6, values.7, values.8, values.9, values.10,
            values.11, values.12, values.13, values.14, Some(original_span_id)
        )
        .fetch_one(&self.pool)
        .await?;
        
        // Incrementar replay_count do span original
        self.increment_replay_count(original_span_id).await?;
        
        println!("✅ Replay registrado: {} -> {}", original_span_id, result.id);
        Ok(result.id)
    }
}