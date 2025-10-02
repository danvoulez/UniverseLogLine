/// # Módulo de Identidades Ghost
/// 
/// Implementa lógica para identidades temporárias que podem ser
/// reivindicadas posteriormente como identidades permanentes.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use std::collections::HashMap;

use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
use crate::motor::span::SpanEmitter;

/// Sistema de gerenciamento de identidades ghost
pub struct GhostIdentitySystem {
    /// Armazenamento de identidades ghost ativas
    ghost_identities: HashMap<String, GhostIdentity>,
    
    /// Configuração do sistema ghost
    config: GhostConfig,
    
    /// Emitente de spans para auditoria
    span_emitter: Option<SpanEmitter>,
}

/// Configuração do sistema de identidades ghost
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostConfig {
    /// Tempo de vida padrão para identidades ghost (em dias)
    pub default_lifetime_days: u32,
    
    /// Máximo de identidades ghost por nó
    pub max_ghosts_per_node: u32,
    
    /// Capacidades limitadas para ghosts
    pub ghost_capabilities: Vec<String>,
    
    /// Requerer verificação para claim
    pub require_verification_for_claim: bool,
    
    /// Auto-limpeza de ghosts expirados
    pub auto_cleanup_expired: bool,
}

/// Identidade ghost temporária
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostIdentity {
    /// LogLine ID da identidade ghost
    pub ghost_id: String,
    
    /// Node que gerou o ghost
    pub issuer_node: String,
    
    /// Timestamp de criação
    pub created_at: DateTime<Utc>,
    
    /// Timestamp de expiração
    pub expires_at: DateTime<Utc>,
    
    /// Status atual do ghost
    pub status: GhostStatus,
    
    /// Chave pública associada
    pub public_key: Vec<u8>,
    
    /// Capacidades do ghost
    pub capabilities: Vec<String>,
    
    /// Dados de sessão associados
    pub session_data: HashMap<String, serde_json::Value>,
    
    /// Spans criados por este ghost
    pub spans_created: Vec<String>,
    
    /// Histórico de atividades
    pub activity_log: Vec<GhostActivity>,
    
    /// Dados para reivindicação futura
    pub claim_hints: Option<ClaimHints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostStatus {
    /// Ghost ativo e utilizável
    Active,
    
    /// Ghost expirado mas ainda não limpo
    Expired,
    
    /// Ghost foi reivindicado com sucesso
    Claimed,
    
    /// Ghost foi revogado antes da expiração
    Revoked,
    
    /// Ghost está sendo processado para claim
    ClaimInProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostActivity {
    /// Timestamp da atividade
    pub timestamp: DateTime<Utc>,
    
    /// Tipo de atividade
    pub activity_type: GhostActivityType,
    
    /// Detalhes da atividade
    pub details: serde_json::Value,
    
    /// Contexto adicional
    pub context: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostActivityType {
    Created,
    SpanCreated,
    DataAccessed,
    InteractionLogged,
    ClaimRequested,
    Expired,
    Revoked,
}

/// Dicas para reivindicação futura
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimHints {
    /// Email sugerido para verificação
    pub suggested_email: Option<String>,
    
    /// Dados biométricos coletados
    pub biometric_data: Option<HashMap<String, String>>,
    
    /// Padrões de comportamento
    pub behavior_patterns: Vec<String>,
    
    /// Preferências do usuário
    pub user_preferences: HashMap<String, serde_json::Value>,
}

/// Resultado de operação ghost
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostOperationResult {
    pub success: bool,
    pub ghost_id: Option<String>,
    pub message: String,
    pub details: HashMap<String, serde_json::Value>,
    pub warnings: Vec<String>,
}

/// Resultado de reivindicação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResult {
    pub success: bool,
    pub new_logline_id: Option<String>,
    pub migrated_spans: u32,
    pub claim_receipt: Option<String>,
    pub error: Option<String>,
}

impl GhostIdentitySystem {
    /// Cria novo sistema de identidades ghost
    pub fn new(config: GhostConfig) -> Self {
        Self {
            ghost_identities: HashMap::new(),
            config,
            span_emitter: None,
        }
    }

    /// Configura emitente de spans
    pub fn with_span_emitter(mut self, span_emitter: SpanEmitter) -> Self {
        self.span_emitter = Some(span_emitter);
        self
    }

    /// Gera nova identidade ghost
    pub async fn generate_ghost_identity(
        &mut self,
        node_name: &str,
        session_data: Option<HashMap<String, serde_json::Value>>,
        custom_lifetime: Option<Duration>,
    ) -> Result<GhostOperationResult, GhostError> {
        // Verifica limite de ghosts por nó
        let node_ghost_count = self.count_active_ghosts_for_node(node_name);
        if node_ghost_count >= self.config.max_ghosts_per_node {
            return Ok(GhostOperationResult {
                success: false,
                ghost_id: None,
                message: format!("Limite de {} ghosts por nó atingido", self.config.max_ghosts_per_node),
                details: HashMap::new(),
                warnings: vec!["Considere fazer claim de ghosts existentes".to_string()],
            });
        }

        // Gera ID único para o ghost
        let ghost_uuid = Uuid::new_v4();
        let ghost_alias = format!("ghost.{}", &ghost_uuid.to_string()[..8]);
        let ghost_id = format!("logline-id://{}/{}", node_name, ghost_alias);

        // Gera chaves criptográficas
        let mut csprng = rand::rngs::OsRng{};
        let signing_key = ed25519_dalek::SigningKey::generate(&mut csprng);
        let public_key = signing_key.verifying_key().to_bytes().to_vec();

        // Define tempo de vida
        let lifetime = custom_lifetime.unwrap_or_else(|| {
            Duration::days(self.config.default_lifetime_days as i64)
        });

        let now = Utc::now();
        let expires_at = now + lifetime;

        let ghost_identity = GhostIdentity {
            ghost_id: ghost_id.clone(),
            issuer_node: node_name.to_string(),
            created_at: now,
            expires_at,
            status: GhostStatus::Active,
            public_key,
            capabilities: self.config.ghost_capabilities.clone(),
            session_data: session_data.unwrap_or_default(),
            spans_created: Vec::new(),
            activity_log: vec![GhostActivity {
                timestamp: now,
                activity_type: GhostActivityType::Created,
                details: serde_json::json!({
                    "generator": "ghost_identity_system",
                    "node": node_name,
                    "lifetime_days": lifetime.num_days()
                }),
                context: None,
            }],
            claim_hints: None,
        };

        // Armazena ghost
        self.ghost_identities.insert(ghost_id.clone(), ghost_identity);

        // Emite span de auditoria
        if let Some(ref emitter) = self.span_emitter {
            self.emit_ghost_created_span(emitter, &ghost_id, node_name).await?;
        }

        Ok(GhostOperationResult {
            success: true,
            ghost_id: Some(ghost_id),
            message: "Identidade ghost criada com sucesso".to_string(),
            details: {
                let mut details = HashMap::new();
                details.insert("expires_at".to_string(), serde_json::json!(expires_at));
                details.insert("capabilities".to_string(), serde_json::json!(self.config.ghost_capabilities));
                details
            },
            warnings: vec![],
        })
    }

    /// Registra atividade de um ghost
    pub async fn log_ghost_activity(
        &mut self,
        ghost_id: &str,
        activity_type: GhostActivityType,
        details: serde_json::Value,
        context: Option<HashMap<String, String>>,
    ) -> Result<(), GhostError> {
        let ghost = self.ghost_identities.get_mut(ghost_id)
            .ok_or_else(|| GhostError::GhostNotFound(ghost_id.to_string()))?;

        // Verifica se ghost ainda está ativo
        if !matches!(ghost.status, GhostStatus::Active) {
            return Err(GhostError::GhostNotActive(ghost_id.to_string()));
        }

        let activity = GhostActivity {
            timestamp: Utc::now(),
            activity_type,
            details,
            context,
        };

        ghost.activity_log.push(activity);

        Ok(())
    }

    /// Reivindicação de identidade ghost
    pub async fn claim_ghost_identity(
        &mut self,
        ghost_id: &str,
        new_alias: &str,
        public_key: Vec<u8>,
        verification_data: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<ClaimResult, GhostError> {
        // Carrega ghost
        let ghost = self.ghost_identities.get_mut(ghost_id)
            .ok_or_else(|| GhostError::GhostNotFound(ghost_id.to_string()))?;

        // Verifica se ghost pode ser reivindicado
        if !matches!(ghost.status, GhostStatus::Active) {
            return Ok(ClaimResult {
                success: false,
                new_logline_id: None,
                migrated_spans: 0,
                claim_receipt: None,
                error: Some("Ghost não está ativo para reivindicação".to_string()),
            });
        }

        // Verifica expiração
        if Utc::now() > ghost.expires_at {
            return Ok(ClaimResult {
                success: false,
                new_logline_id: None,
                migrated_spans: 0,
                claim_receipt: None,
                error: Some("Ghost expirado".to_string()),
            });
        }

        // Valida dados de verificação se necessário
        if self.config.require_verification_for_claim {
            if verification_data.is_none() {
                return Ok(ClaimResult {
                    success: false,
                    new_logline_id: None,
                    migrated_spans: 0,
                    claim_receipt: None,
                    error: Some("Dados de verificação são obrigatórios".to_string()),
                });
            }
        }

        // Extrai node do ghost_id
        let node_name = self.extract_node_from_ghost_id(ghost_id)?;

        // Cria nova identidade permanente
        let new_logline_id = format!("logline-id://{}/{}", node_name, new_alias);

        // Atualiza status do ghost
        ghost.status = GhostStatus::ClaimInProgress;

        // Simula migração de spans
        let migrated_spans = ghost.spans_created.len() as u32;

        // Atualiza log de atividade
        ghost.activity_log.push(GhostActivity {
            timestamp: Utc::now(),
            activity_type: GhostActivityType::ClaimRequested,
            details: serde_json::json!({
                "new_alias": new_alias,
                "new_logline_id": new_logline_id,
                "verification_provided": verification_data.is_some()
            }),
            context: None,
        });

        // Marca como reivindicado
        ghost.status = GhostStatus::Claimed;

        // Gera recibo de reivindicação
        let claim_receipt = format!("claim-receipt-{}", Uuid::new_v4());

        // Emite span de auditoria
        if let Some(ref emitter) = self.span_emitter {
            self.emit_ghost_claimed_span(emitter, ghost_id, &new_logline_id, migrated_spans).await?;
        }

        Ok(ClaimResult {
            success: true,
            new_logline_id: Some(new_logline_id),
            migrated_spans,
            claim_receipt: Some(claim_receipt),
            error: None,
        })
    }

    /// Obtém informações de um ghost
    pub fn get_ghost_info(&self, ghost_id: &str) -> Option<&GhostIdentity> {
        self.ghost_identities.get(ghost_id)
    }

    /// Lista ghosts ativos para um nó
    pub fn list_active_ghosts_for_node(&self, node_name: &str) -> Vec<&GhostIdentity> {
        self.ghost_identities
            .values()
            .filter(|ghost| {
                ghost.issuer_node == node_name && 
                matches!(ghost.status, GhostStatus::Active) &&
                Utc::now() <= ghost.expires_at
            })
            .collect()
    }

    /// Limpa ghosts expirados
    pub async fn cleanup_expired_ghosts(&mut self) -> Result<u32, GhostError> {
        let now = Utc::now();
        let mut cleaned_count = 0;

        let expired_ghost_ids: Vec<String> = self.ghost_identities
            .iter()
            .filter_map(|(id, ghost)| {
                if now > ghost.expires_at && !matches!(ghost.status, GhostStatus::Claimed) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        for ghost_id in expired_ghost_ids {
            if let Some(mut ghost) = self.ghost_identities.get_mut(&ghost_id) {
                ghost.status = GhostStatus::Expired;
                ghost.activity_log.push(GhostActivity {
                    timestamp: now,
                    activity_type: GhostActivityType::Expired,
                    details: serde_json::json!({"cleanup_reason": "automatic_expiration"}),
                    context: None,
                });
            }

            if self.config.auto_cleanup_expired {
                self.ghost_identities.remove(&ghost_id);
                cleaned_count += 1;
            }
        }

        Ok(cleaned_count)
    }

    /// Conta ghosts ativos para um nó
    fn count_active_ghosts_for_node(&self, node_name: &str) -> u32 {
        self.list_active_ghosts_for_node(node_name).len() as u32
    }

    /// Extrai node name do ghost_id
    fn extract_node_from_ghost_id(&self, ghost_id: &str) -> Result<String, GhostError> {
        let parts: Vec<&str> = ghost_id.split('/').collect();
        if parts.len() != 2 || !parts[0].starts_with("logline-id://") {
            return Err(GhostError::InvalidGhostID(ghost_id.to_string()));
        }
        
        Ok(parts[0].trim_start_matches("logline-id://").to_string())
    }

    /// Emite span de criação de ghost
    async fn emit_ghost_created_span(
        &self,
        emitter: &SpanEmitter,
        ghost_id: &str,
        node_name: &str,
    ) -> Result<(), GhostError> {
        let span_data = serde_json::json!({
            "operation": "ghost_identity_created",
            "ghost_id": ghost_id,
            "issuer_node": node_name,
            "capabilities": self.config.ghost_capabilities,
            "lifetime_days": self.config.default_lifetime_days
        });

        // TODO: Usar LogLineID real do sistema
        let system_id = crate::infra::id::LogLineIDWithKeys::generate_new()
            .map_err(|e| GhostError::SystemError(e.to_string()))?;

        emitter.emit_span(
            "ghost_identity_operation",
            "ghost_system",
            &system_id,
            Some(span_data),
        ).await
        .map_err(|e| GhostError::SpanEmissionError(e.to_string()))?;

        Ok(())
    }

    /// Emite span de reivindicação de ghost
    async fn emit_ghost_claimed_span(
        &self,
        emitter: &SpanEmitter,
        ghost_id: &str,
        new_logline_id: &str,
        migrated_spans: u32,
    ) -> Result<(), GhostError> {
        let span_data = serde_json::json!({
            "operation": "ghost_identity_claimed",
            "ghost_id": ghost_id,
            "new_logline_id": new_logline_id,
            "migrated_spans": migrated_spans,
            "claim_timestamp": Utc::now()
        });

        // TODO: Usar LogLineID real do sistema
        let system_id = crate::infra::id::LogLineIDWithKeys::generate_new()
            .map_err(|e| GhostError::SystemError(e.to_string()))?;

        emitter.emit_span(
            "ghost_claim_operation",
            "ghost_system",
            &system_id,
            Some(span_data),
        ).await
        .map_err(|e| GhostError::SpanEmissionError(e.to_string()))?;

        Ok(())
    }
}

impl Default for GhostConfig {
    fn default() -> Self {
        Self {
            default_lifetime_days: 30,
            max_ghosts_per_node: 10,
            ghost_capabilities: vec![
                "create_spans".to_string(),
                "read_basic_data".to_string(),
            ],
            require_verification_for_claim: true,
            auto_cleanup_expired: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GhostError {
    #[error("Ghost não encontrado: {0}")]
    GhostNotFound(String),
    
    #[error("Ghost não está ativo: {0}")]
    GhostNotActive(String),
    
    #[error("Ghost ID inválido: {0}")]
    InvalidGhostID(String),
    
    #[error("Erro do sistema: {0}")]
    SystemError(String),
    
    #[error("Erro na emissão de span: {0}")]
    SpanEmissionError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ghost_system_creation() {
        let config = GhostConfig::default();
        let system = GhostIdentitySystem::new(config);
        assert_eq!(system.ghost_identities.len(), 0);
    }

    #[tokio::test]
    async fn test_ghost_identity_generation() {
        let config = GhostConfig::default();
        let mut system = GhostIdentitySystem::new(config);
        
        let result = system.generate_ghost_identity(
            "test-node",
            None,
            None,
        ).await;
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.ghost_id.is_some());
        
        let ghost_id = result.ghost_id.unwrap();
        assert!(ghost_id.starts_with("logline-id://test-node/ghost."));
    }

    #[tokio::test]
    async fn test_ghost_activity_logging() {
        let config = GhostConfig::default();
        let mut system = GhostIdentitySystem::new(config);
        
        // Cria ghost
        let result = system.generate_ghost_identity("test-node", None, None).await.unwrap();
        let ghost_id = result.ghost_id.unwrap();
        
        // Log atividade
        let log_result = system.log_ghost_activity(
            &ghost_id,
            GhostActivityType::SpanCreated,
            serde_json::json!({"span_id": "test-span"}),
            None,
        ).await;
        
        assert!(log_result.is_ok());
        
        // Verifica se foi logado
        let ghost = system.get_ghost_info(&ghost_id).unwrap();
        assert_eq!(ghost.activity_log.len(), 2); // Created + SpanCreated
    }
}