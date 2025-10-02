use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use crate::motor::types::{Span, SpanStatus, SpanType, Visibility};
use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
use crate::timeline::timeline::{Timeline, TimelineEntry, TimelineQuery};

/// Tenant-specific timeline wrapper
pub struct TenantTimeline<'a> {
    timeline: &'a Timeline,
    tenant_id: String,
    user_id: Option<Uuid>,
    organization_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TenantSpanOptions {
    pub visibility: Option<Visibility>,
    pub span_type: Option<SpanType>,
    pub metadata: Option<serde_json::Value>,
}

impl Default for TenantSpanOptions {
    fn default() -> Self {
        Self {
            visibility: Some(Visibility::Private),
            span_type: Some(SpanType::User),
            metadata: Some(serde_json::Value::Object(serde_json::Map::new())),
        }
    }
}

impl<'a> TenantTimeline<'a> {
    /// Creates a new tenant-specific timeline
    pub fn new(
        timeline: &'a Timeline, 
        tenant_id: &str,
        user_id: Option<Uuid>,
        organization_id: Option<Uuid>,
    ) -> Self {
        TenantTimeline {
            timeline,
            tenant_id: tenant_id.to_string(),
            user_id,
            organization_id,
        }
    }
    
    /// Creates a span with tenant context
    pub async fn create_span(
        &self,
        title: &str,
        id_with_keys: &LogLineIDWithKeys,
        payload: serde_json::Value,
        options: Option<TenantSpanOptions>,
    ) -> Result<Uuid, sqlx::Error> {
        // Create the base span
        let mut span = Span::new(id_with_keys.id.to_string(), title.to_string())
            .with_payload(payload);
            
        // Add tenant context
        span.tenant_id = Some(self.tenant_id.clone());
        span.organization_id = self.organization_id;
        span.user_id = self.user_id;
        
        // Apply options if provided
        if let Some(opts) = options {
            span.visibility = opts.visibility;
            span.span_type = opts.span_type;
            span.metadata = opts.metadata;
        }
        
        // Sign and append the span
        self.timeline.append_signed_span(&mut span, id_with_keys).await
    }
    
    /// List spans for this tenant with filtering
    pub async fn list_spans(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        let query = TimelineQuery {
            logline_id: None,
            contract_id: None,
            workflow_id: None,
            limit,
            offset,
            tenant_id: Some(self.tenant_id.clone()),
            organization_id: self.organization_id,
            user_id: self.user_id,
            span_type: None,
            visibility: None,
        };
        
        self.timeline.list_spans(&query).await
    }
    
    /// Search spans for this tenant
    pub async fn search_spans(
        &self,
        search_term: &str,
        limit: Option<i64>,
    ) -> Result<Vec<TimelineEntry>, sqlx::Error> {
        self.timeline.search_spans(search_term, Some(&self.tenant_id), limit).await
    }
    
    /// Get tenant statistics
    pub async fn get_stats(&self) -> Result<serde_json::Value, sqlx::Error> {
        self.timeline.get_tenant_stats(&self.tenant_id).await
    }
}

/// Factory function to create a tenant timeline
pub fn create_tenant_timeline<'a>(
    timeline: &'a Timeline,
    tenant_id: &str,
    user_id: Option<Uuid>,
    organization_id: Option<Uuid>,
) -> TenantTimeline<'a> {
    TenantTimeline::new(timeline, tenant_id, user_id, organization_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::id::logline_id::LogLineID;
    
    #[tokio::test]
    async fn test_tenant_timeline() {
        // This test would require a database connection
        // So we'll just outline the test structure
        
        /*
        // Setup
        let database_url = "postgresql://localhost/logline_test";
        let timeline = Timeline::new(database_url).await.unwrap();
        
        let tenant_id = "test-tenant";
        let user_id = Uuid::new_v4();
        let tenant_timeline = create_tenant_timeline(&timeline, tenant_id, Some(user_id), None);
        
        // Generate an identity for testing
        let id_with_keys = LogLineID::generate("test-node");
        
        // Create a span
        let payload = serde_json::json!({
            "test": "data",
            "value": 42
        });
        
        let options = TenantSpanOptions {
            visibility: Some(Visibility::Organization),
            ..Default::default()
        };
        
        let span_id = tenant_timeline.create_span(
            "Test Tenant Span",
            &id_with_keys,
            payload,
            Some(options)
        ).await.unwrap();
        
        // Get the span
        let entries = tenant_timeline.list_spans(Some(10), None).await.unwrap();
        assert!(!entries.is_empty());
        
        // Check tenant context
        let first_span = &entries[0];
        assert_eq!(first_span.tenant_id, Some(tenant_id.to_string()));
        assert_eq!(first_span.user_id, Some(user_id));
        */
    }
}