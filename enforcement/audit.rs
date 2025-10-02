use crate::timeline::{Span, Timeline};
use crate::rules::EnforcementAction;
use crate::enforcement::Enforcer;
use std::sync::{Arc, Mutex};
use serde_json::{json, Value};
use chrono::{Utc, DateTime};
use std::collections::HashMap;

/// Tipo de evento de auditoria
#[derive(Debug, Clone)]
pub enum AuditEvent {
    /// Span permitido
    SpanAllowed {
        span_id: String,
        channel: String,
        actor: Option<String>,
        timestamp: DateTime<Utc>,
        details: Option<String>,
    },
    
    /// Span rejeitado
    SpanRejected {
        span_id: String,
        channel: String,
        actor: Option<String>,
        timestamp: DateTime<Utc>,
        reason: String,
        details: Option<String>,
    },
    
    /// Alteração de configuração
    ConfigChanged {
        actor: Option<String>,
        timestamp: DateTime<Utc>,
        setting: String,
        old_value: Option<String>,
        new_value: Option<String>,
    },
    
    /// Acesso realizado
    AccessEvent {
        actor: Option<String>,
        timestamp: DateTime<Utc>,
        resource: String,
        operation: String,
        success: bool,
        details: Option<String>,
    },
}

impl AuditEvent {
    /// Converte o evento de auditoria para um span
    pub fn to_span(&self, audit_channel: &str) -> Span {
        let (id, data) = match self {
            AuditEvent::SpanAllowed { span_id, channel, actor, timestamp, details } => {
                let data = json!({
                    "type": "audit.span_allowed",
                    "span_id": span_id,
                    "channel": channel,
                    "actor": actor,
                    "timestamp": timestamp.to_rfc3339(),
                    "details": details
                });
                
                (format!("audit-allow-{}", span_id), data)
            },
            
            AuditEvent::SpanRejected { span_id, channel, actor, timestamp, reason, details } => {
                let data = json!({
                    "type": "audit.span_rejected",
                    "span_id": span_id,
                    "channel": channel,
                    "actor": actor,
                    "timestamp": timestamp.to_rfc3339(),
                    "reason": reason,
                    "details": details
                });
                
                (format!("audit-reject-{}", span_id), data)
            },
            
            AuditEvent::ConfigChanged { actor, timestamp, setting, old_value, new_value } => {
                let data = json!({
                    "type": "audit.config_changed",
                    "actor": actor,
                    "timestamp": timestamp.to_rfc3339(),
                    "setting": setting,
                    "old_value": old_value,
                    "new_value": new_value
                });
                
                (format!("audit-config-{}", Utc::now().timestamp_millis()), data)
            },
            
            AuditEvent::AccessEvent { actor, timestamp, resource, operation, success, details } => {
                let data = json!({
                    "type": "audit.access",
                    "actor": actor,
                    "timestamp": timestamp.to_rfc3339(),
                    "resource": resource,
                    "operation": operation,
                    "success": success,
                    "details": details
                });
                
                (format!("audit-access-{}", Utc::now().timestamp_millis()), data)
            },
        };
        
        Span {
            id,
            channel: audit_channel.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::to_string(&data).unwrap()),
            tenant_id: None,
        }
    }
}

/// Armazenamento para eventos de auditoria
pub trait AuditStorage: Send + Sync {
    /// Armazena um evento de auditoria
    fn store_event(&mut self, event: AuditEvent) -> Result<(), String>;
    
    /// Consulta eventos de auditoria
    fn query_events(&self, 
        filter: Option<&HashMap<String, String>>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>
    ) -> Result<Vec<AuditEvent>, String>;
}

/// Armazenamento de auditoria baseado na timeline
pub struct TimelineAuditStorage {
    /// Timeline onde eventos serão armazenados
    timeline: Arc<Mutex<dyn Timeline>>,
    
    /// Canal para eventos de auditoria
    audit_channel: String,
}

impl TimelineAuditStorage {
    /// Cria um novo armazenamento de auditoria baseado na timeline
    pub fn new(timeline: Arc<Mutex<dyn Timeline>>, audit_channel: &str) -> Self {
        Self {
            timeline,
            audit_channel: audit_channel.to_string(),
        }
    }
    
    /// Converte um span de auditoria de volta para um evento
    fn span_to_audit_event(&self, span: &Span) -> Result<AuditEvent, String> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                let event_type = json.get("type")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| "Tipo de evento de auditoria não encontrado".to_string())?;
                
                match event_type {
                    "audit.span_allowed" => {
                        let span_id = json.get("span_id")
                            .and_then(|id| id.as_str())
                            .ok_or_else(|| "ID do span não encontrado".to_string())?;
                            
                        let channel = json.get("channel")
                            .and_then(|c| c.as_str())
                            .ok_or_else(|| "Canal não encontrado".to_string())?;
                            
                        let actor = json.get("actor")
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string());
                            
                        let timestamp_str = json.get("timestamp")
                            .and_then(|t| t.as_str())
                            .ok_or_else(|| "Timestamp não encontrado".to_string())?;
                            
                        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| format!("Erro ao analisar timestamp: {}", e))?;
                            
                        let details = json.get("details")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string());
                        
                        Ok(AuditEvent::SpanAllowed {
                            span_id: span_id.to_string(),
                            channel: channel.to_string(),
                            actor,
                            timestamp,
                            details,
                        })
                    },
                    
                    "audit.span_rejected" => {
                        let span_id = json.get("span_id")
                            .and_then(|id| id.as_str())
                            .ok_or_else(|| "ID do span não encontrado".to_string())?;
                            
                        let channel = json.get("channel")
                            .and_then(|c| c.as_str())
                            .ok_or_else(|| "Canal não encontrado".to_string())?;
                            
                        let actor = json.get("actor")
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string());
                            
                        let timestamp_str = json.get("timestamp")
                            .and_then(|t| t.as_str())
                            .ok_or_else(|| "Timestamp não encontrado".to_string())?;
                            
                        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| format!("Erro ao analisar timestamp: {}", e))?;
                            
                        let reason = json.get("reason")
                            .and_then(|r| r.as_str())
                            .ok_or_else(|| "Razão não encontrada".to_string())?;
                            
                        let details = json.get("details")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string());
                        
                        Ok(AuditEvent::SpanRejected {
                            span_id: span_id.to_string(),
                            channel: channel.to_string(),
                            actor,
                            timestamp,
                            reason: reason.to_string(),
                            details,
                        })
                    },
                    
                    "audit.config_changed" => {
                        let actor = json.get("actor")
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string());
                            
                        let timestamp_str = json.get("timestamp")
                            .and_then(|t| t.as_str())
                            .ok_or_else(|| "Timestamp não encontrado".to_string())?;
                            
                        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| format!("Erro ao analisar timestamp: {}", e))?;
                            
                        let setting = json.get("setting")
                            .and_then(|s| s.as_str())
                            .ok_or_else(|| "Configuração não encontrada".to_string())?;
                            
                        let old_value = json.get("old_value")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                            
                        let new_value = json.get("new_value")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        
                        Ok(AuditEvent::ConfigChanged {
                            actor,
                            timestamp,
                            setting: setting.to_string(),
                            old_value,
                            new_value,
                        })
                    },
                    
                    "audit.access" => {
                        let actor = json.get("actor")
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string());
                            
                        let timestamp_str = json.get("timestamp")
                            .and_then(|t| t.as_str())
                            .ok_or_else(|| "Timestamp não encontrado".to_string())?;
                            
                        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| format!("Erro ao analisar timestamp: {}", e))?;
                            
                        let resource = json.get("resource")
                            .and_then(|r| r.as_str())
                            .ok_or_else(|| "Recurso não encontrado".to_string())?;
                            
                        let operation = json.get("operation")
                            .and_then(|o| o.as_str())
                            .ok_or_else(|| "Operação não encontrada".to_string())?;
                            
                        let success = json.get("success")
                            .and_then(|s| s.as_bool())
                            .ok_or_else(|| "Status de sucesso não encontrado".to_string())?;
                            
                        let details = json.get("details")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string());
                        
                        Ok(AuditEvent::AccessEvent {
                            actor,
                            timestamp,
                            resource: resource.to_string(),
                            operation: operation.to_string(),
                            success,
                            details,
                        })
                    },
                    
                    _ => Err(format!("Tipo de evento de auditoria desconhecido: {}", event_type)),
                }
            } else {
                Err("Dados de auditoria inválidos".to_string())
            }
        } else {
            Err("Span de auditoria sem dados".to_string())
        }
    }
}

impl AuditStorage for TimelineAuditStorage {
    fn store_event(&mut self, event: AuditEvent) -> Result<(), String> {
        let span = event.to_span(&self.audit_channel);
        
        if let Ok(mut timeline) = self.timeline.lock() {
            timeline.append(&span)
                .map_err(|e| format!("Erro ao armazenar evento de auditoria: {}", e))
        } else {
            Err("Não foi possível obter acesso à timeline".to_string())
        }
    }
    
    fn query_events(
        &self,
        filter: Option<&HashMap<String, String>>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>, String> {
        if let Ok(timeline) = self.timeline.lock() {
            // Consultar spans no canal de auditoria
            let spans = timeline.query(
                Some(&self.audit_channel),
                None, // Timestamp será filtrado depois
                limit,
            ).map_err(|e| format!("Erro ao consultar eventos de auditoria: {}", e))?;
            
            // Filtrar por timestamp
            let spans = spans.into_iter()
                .filter(|span| {
                    if let Some(start) = start_time {
                        if span.timestamp < start {
                            return false;
                        }
                    }
                    
                    if let Some(end) = end_time {
                        if span.timestamp > end {
                            return false;
                        }
                    }
                    
                    true
                })
                .collect::<Vec<_>>();
            
            // Converter para eventos de auditoria
            let mut events = Vec::new();
            for span in spans {
                match self.span_to_audit_event(&span) {
                    Ok(event) => {
                        // Aplicar filtros adicionais
                        if let Some(filters) = filter {
                            let mut include = true;
                            
                            // Verificar se o evento corresponde a todos os filtros
                            for (key, value) in filters {
                                let matches = match &event {
                                    AuditEvent::SpanAllowed { span_id, channel, actor, .. } => {
                                        match key.as_str() {
                                            "span_id" => span_id == value,
                                            "channel" => channel == value,
                                            "actor" => actor.as_ref().map_or(false, |a| a == value),
                                            "type" => "span_allowed" == value,
                                            _ => false,
                                        }
                                    },
                                    AuditEvent::SpanRejected { span_id, channel, actor, reason, .. } => {
                                        match key.as_str() {
                                            "span_id" => span_id == value,
                                            "channel" => channel == value,
                                            "actor" => actor.as_ref().map_or(false, |a| a == value),
                                            "reason" => reason.contains(value),
                                            "type" => "span_rejected" == value,
                                            _ => false,
                                        }
                                    },
                                    AuditEvent::ConfigChanged { setting, actor, .. } => {
                                        match key.as_str() {
                                            "setting" => setting == value,
                                            "actor" => actor.as_ref().map_or(false, |a| a == value),
                                            "type" => "config_changed" == value,
                                            _ => false,
                                        }
                                    },
                                    AuditEvent::AccessEvent { actor, resource, operation, success, .. } => {
                                        match key.as_str() {
                                            "resource" => resource == value,
                                            "operation" => operation == value,
                                            "actor" => actor.as_ref().map_or(false, |a| a == value),
                                            "success" => success.to_string() == value,
                                            "type" => "access" == value,
                                            _ => false,
                                        }
                                    },
                                };
                                
                                if !matches {
                                    include = false;
                                    break;
                                }
                            }
                            
                            if include {
                                events.push(event);
                            }
                        } else {
                            events.push(event);
                        }
                    },
                    Err(_) => continue, // Ignorar eventos inválidos
                }
            }
            
            Ok(events)
        } else {
            Err("Não foi possível obter acesso à timeline".to_string())
        }
    }
}

/// Enforcer que registra decisões de enforcement para auditoria
pub struct AuditingEnforcer<T: Enforcer> {
    /// Enforcer interno
    inner: T,
    
    /// Armazenamento de auditoria
    audit_storage: Arc<Mutex<dyn AuditStorage>>,
    
    /// Identidade atual
    current_identity: Option<String>,
}

impl<T: Enforcer> AuditingEnforcer<T> {
    /// Cria um novo enforcer com auditoria
    pub fn new(inner: T, audit_storage: Arc<Mutex<dyn AuditStorage>>) -> Self {
        Self {
            inner,
            audit_storage,
            current_identity: None,
        }
    }
    
    /// Define a identidade atual
    pub fn set_current_identity(&mut self, identity: Option<String>) {
        self.current_identity = identity;
    }
    
    /// Registra um evento de auditoria
    fn record_audit_event(&self, event: AuditEvent) {
        if let Ok(mut storage) = self.audit_storage.lock() {
            let _ = storage.store_event(event);
        }
    }
}

impl<T: Enforcer> Enforcer for AuditingEnforcer<T> {
    fn enforce(&self, span: &Span) -> EnforcementAction {
        // Aplicar enforcer interno
        let action = self.inner.enforce(span);
        
        // Registrar evento de auditoria
        match &action {
            EnforcementAction::Allow => {
                self.record_audit_event(AuditEvent::SpanAllowed {
                    span_id: span.id.clone(),
                    channel: span.channel.clone(),
                    actor: self.current_identity.clone(),
                    timestamp: Utc::now(),
                    details: None,
                });
            },
            EnforcementAction::Reject(reason) => {
                self.record_audit_event(AuditEvent::SpanRejected {
                    span_id: span.id.clone(),
                    channel: span.channel.clone(),
                    actor: self.current_identity.clone(),
                    timestamp: Utc::now(),
                    reason: reason.clone(),
                    details: None,
                });
            },
        }
        
        action
    }
}