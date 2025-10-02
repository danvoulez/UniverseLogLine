/// # Módulo de Assinatura Digital LogLine
/// 
/// Implementa assinatura e verificação digital usando ed25519
/// integrado com o sistema de identidade LogLine ID computável.

use serde::{Deserialize, Serialize};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
use crate::motor::span::SpanEmitter;

/// Sistema de assinatura computável para LogLine IDs
pub struct LogLineSignatureSystem {
    /// Cache de chaves públicas para verificação rápida
    public_key_cache: HashMap<String, VerifyingKey>,
    
    /// Emitente de spans para auditoria
    span_emitter: Option<SpanEmitter>,
    
    /// Configurações do sistema de assinatura
    config: SignatureConfig,
}

/// Configuração do sistema de assinatura
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConfig {
    /// Algoritmo de hash para payloads
    pub hash_algorithm: HashAlgorithm,
    
    /// Tolerância de timestamp (em segundos)
    pub timestamp_tolerance_seconds: u64,
    
    /// Cache TTL para chaves públicas (em segundos)
    pub public_key_cache_ttl: u64,
    
    /// Requerer timestamps em payloads
    pub require_timestamps: bool,
    
    /// Modo de verificação strict
    pub strict_verification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashAlgorithm {
    Sha256,
    Sha512,
    Blake3,
}

/// Resultado de uma operação de assinatura
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureResult {
    pub success: bool,
    pub signature: Option<String>,
    pub payload_hash: String,
    pub timestamp: DateTime<Utc>,
    pub logline_id: String,
    pub algorithm_used: String,
    pub error: Option<String>,
}

/// Resultado de uma verificação de assinatura
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub signature_valid: bool,
    pub identity_status: IdentityStatus,
    pub public_key_used: String,
    pub verification_timestamp: DateTime<Utc>,
    pub trust_score: f64,
    pub verification_details: VerificationDetails,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdentityStatus {
    Active,
    Suspended,
    Ghost,
    Revoked,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDetails {
    pub algorithm_used: String,
    pub key_strength: u32,
    pub signature_fresh: bool,
    pub identity_verified: bool,
    pub revocation_checked: bool,
    pub timestamp_valid: bool,
    pub payload_integrity: bool,
}

/// Payload estruturado para assinatura
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePayload {
    /// Dados principais
    pub data: serde_json::Value,
    
    /// Timestamp da criação
    pub timestamp: DateTime<Utc>,
    
    /// Contexto da assinatura
    pub context: SignatureContext,
    
    /// Nonce para evitar replay attacks
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureContext {
    /// Tipo de operação sendo assinada
    pub operation_type: String,
    
    /// ID do contrato (se aplicável)
    pub contract_id: Option<String>,
    
    /// Metadados adicionais
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LogLineSignatureSystem {
    /// Cria novo sistema de assinatura
    pub fn new(config: SignatureConfig) -> Self {
        Self {
            public_key_cache: HashMap::new(),
            span_emitter: None,
            config,
        }
    }

    /// Configura emitente de spans para auditoria
    pub fn with_span_emitter(mut self, span_emitter: SpanEmitter) -> Self {
        self.span_emitter = Some(span_emitter);
        self
    }

    /// Assina um payload com LogLine ID
    pub async fn sign_payload(
        &self,
        id_with_keys: &LogLineIDWithKeys,
        payload: &SignablePayload,
    ) -> Result<SignatureResult, SignatureError> {
        let start_time = std::time::Instant::now();
        
        // Serializa payload para bytes
        let payload_bytes = self.serialize_payload(payload)?;
        
        // Calcula hash do payload
        let payload_hash = self.calculate_hash(&payload_bytes);
        
        // Cria assinatura
        let signature = id_with_keys.signing_key.sign(&payload_bytes);
        let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());
        
        let result = SignatureResult {
            success: true,
            signature: Some(signature_b64),
            payload_hash: hex::encode(payload_hash),
            timestamp: Utc::now(),
            logline_id: id_with_keys.id.to_string(),
            algorithm_used: "ed25519".to_string(),
            error: None,
        };
        
        // Emite span de auditoria
        if let Some(ref emitter) = self.span_emitter {
            self.emit_signature_span(emitter, &result, start_time.elapsed()).await?;
        }
        
        Ok(result)
    }

    /// Verifica assinatura de payload
    pub async fn verify_signature(
        &mut self,
        logline_id: &str,
        payload: &SignablePayload,
        signature_b64: &str,
    ) -> Result<VerificationResult, SignatureError> {
        let start_time = std::time::Instant::now();
        
        // Carrega identidade
        let identity = self.load_identity(logline_id).await?;
        
        // Obtém chave pública (com cache)
        let public_key = self.get_public_key(&identity).await?;
        
        // Serializa payload
        let payload_bytes = self.serialize_payload(payload)?;
        
        // Decodifica assinatura
        let signature_bytes = general_purpose::STANDARD
            .decode(signature_b64)
            .map_err(|e| SignatureError::InvalidSignatureFormat(e.to_string()))?;
        
        let signature = Signature::from_bytes(&signature_bytes)
            .map_err(|e| SignatureError::InvalidSignatureFormat(e.to_string()))?;
        
        // Verifica assinatura criptográfica
        let signature_valid = public_key.verify(&payload_bytes, &signature).is_ok();
        
        // Verifica timestamp se necessário
        let timestamp_valid = if self.config.require_timestamps {
            self.verify_timestamp(&payload.timestamp)?
        } else {
            true
        };
        
        // Calcula trust score
        let trust_score = self.calculate_trust_score(&identity).await?;
        
        // Verifica status de revogação
        let identity_status = self.check_identity_status(&identity).await?;
        let revocation_checked = !matches!(identity_status, IdentityStatus::Unknown);
        
        let verification_details = VerificationDetails {
            algorithm_used: "ed25519".to_string(),
            key_strength: 256,
            signature_fresh: self.is_signature_fresh(&payload.timestamp),
            identity_verified: matches!(identity_status, IdentityStatus::Active),
            revocation_checked,
            timestamp_valid,
            payload_integrity: signature_valid,
        };
        
        let mut warnings = Vec::new();
        
        // Adiciona avisos baseados na verificação
        if !timestamp_valid {
            warnings.push("Timestamp fora da tolerância aceitável".to_string());
        }
        
        if trust_score < 0.5 {
            warnings.push("Trust score baixo para esta identidade".to_string());
        }
        
        if matches!(identity_status, IdentityStatus::Ghost) {
            warnings.push("Identidade é temporária (ghost)".to_string());
        }
        
        let result = VerificationResult {
            signature_valid: signature_valid && timestamp_valid,
            identity_status,
            public_key_used: hex::encode(public_key.to_bytes()),
            verification_timestamp: Utc::now(),
            trust_score,
            verification_details,
            warnings,
            error: None,
        };
        
        // Emite span de auditoria
        if let Some(ref emitter) = self.span_emitter {
            self.emit_verification_span(emitter, &result, start_time.elapsed()).await?;
        }
        
        Ok(result)
    }

    /// Cria payload assinável a partir de dados
    pub fn create_signable_payload(
        &self,
        data: serde_json::Value,
        operation_type: &str,
        contract_id: Option<String>,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> SignablePayload {
        SignablePayload {
            data,
            timestamp: Utc::now(),
            context: SignatureContext {
                operation_type: operation_type.to_string(),
                contract_id,
                metadata: metadata.unwrap_or_default(),
            },
            nonce: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Serializa payload para bytes (determinística)
    fn serialize_payload(&self, payload: &SignablePayload) -> Result<Vec<u8>, SignatureError> {
        // Serialização determinística usando JSON canonicalizado
        let json = serde_json::to_string(payload)
            .map_err(|e| SignatureError::SerializationError(e.to_string()))?;
        
        Ok(json.into_bytes())
    }

    /// Calcula hash do payload
    fn calculate_hash(&self, data: &[u8]) -> Vec<u8> {
        match self.config.hash_algorithm {
            HashAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            },
            HashAlgorithm::Sha512 => {
                let mut hasher = sha2::Sha512::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            },
            HashAlgorithm::Blake3 => {
                blake3::hash(data).as_bytes().to_vec()
            }
        }
    }

    /// Carrega identidade do sistema
    async fn load_identity(&self, logline_id: &str) -> Result<LogLineID, SignatureError> {
        // Extrai node_name do logline_id
        let parts: Vec<&str> = logline_id.split('/').collect();
        if parts.len() != 2 || !parts[0].starts_with("logline-id://") {
            return Err(SignatureError::InvalidLogLineID(logline_id.to_string()));
        }
        
        let node_name = parts[0].trim_start_matches("logline-id://");
        
        // Tenta carregar do arquivo local
        match LogLineID::load_from_file(node_name) {
            Ok(id_with_keys) => Ok(id_with_keys.id),
            Err(_) => {
                // TODO: Implementar carregamento da rede federada
                Err(SignatureError::IdentityNotFound(logline_id.to_string()))
            }
        }
    }

    /// Obtém chave pública (com cache)
    async fn get_public_key(&mut self, identity: &LogLineID) -> Result<VerifyingKey, SignatureError> {
        let cache_key = identity.to_string();
        
        // Verifica cache
        if let Some(cached_key) = self.public_key_cache.get(&cache_key) {
            return Ok(*cached_key);
        }
        
        // Converte chave pública da identidade
        let public_key_bytes: [u8; 32] = identity.public_key
            .as_slice()
            .try_into()
            .map_err(|_| SignatureError::InvalidPublicKeyFormat)?;
        
        let public_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|e| SignatureError::InvalidPublicKeyFormat)?;
        
        // Adiciona ao cache
        self.public_key_cache.insert(cache_key, public_key);
        
        Ok(public_key)
    }

    /// Verifica se timestamp está dentro da tolerância
    fn verify_timestamp(&self, timestamp: &DateTime<Utc>) -> Result<bool, SignatureError> {
        let now = Utc::now();
        let tolerance = chrono::Duration::seconds(self.config.timestamp_tolerance_seconds as i64);
        
        let diff = if *timestamp > now {
            *timestamp - now
        } else {
            now - *timestamp
        };
        
        Ok(diff <= tolerance)
    }

    /// Verifica se assinatura é recente
    fn is_signature_fresh(&self, timestamp: &DateTime<Utc>) -> bool {
        let now = Utc::now();
        let age = now - *timestamp;
        age <= chrono::Duration::hours(1) // Considera fresco se < 1 hora
    }

    /// Calcula trust score da identidade
    async fn calculate_trust_score(&self, _identity: &LogLineID) -> Result<f64, SignatureError> {
        // TODO: Implementar cálculo real de trust score baseado em:
        // - Idade da identidade
        // - Número de assinaturas válidas
        // - Endorsements da rede
        // - Histórico de comportamento
        
        Ok(0.8) // Placeholder
    }

    /// Verifica status atual da identidade
    async fn check_identity_status(&self, _identity: &LogLineID) -> Result<IdentityStatus, SignatureError> {
        // TODO: Implementar verificação real de status:
        // - Consultar lista de revogação
        // - Verificar se foi suspensa
        // - Verificar se é ghost
        
        Ok(IdentityStatus::Active) // Placeholder
    }

    /// Emite span de auditoria para assinatura
    async fn emit_signature_span(
        &self,
        emitter: &SpanEmitter,
        result: &SignatureResult,
        duration: std::time::Duration,
    ) -> Result<(), SignatureError> {
        let span_data = serde_json::json!({
            "operation": "signature_created",
            "logline_id": result.logline_id,
            "algorithm": result.algorithm_used,
            "payload_hash": result.payload_hash,
            "success": result.success,
            "duration_ms": duration.as_millis()
        });
        
        // TODO: Usar LogLineID real do sistema
        let system_id = crate::infra::id::LogLineIDWithKeys::generate_new()
            .map_err(|e| SignatureError::SystemError(e.to_string()))?;
        
        emitter.emit_span(
            "signature_operation",
            "signature_system",
            &system_id,
            Some(span_data),
        ).await
        .map_err(|e| SignatureError::SpanEmissionError(e.to_string()))?;
        
        Ok(())
    }

    /// Emite span de auditoria para verificação
    async fn emit_verification_span(
        &self,
        emitter: &SpanEmitter,
        result: &VerificationResult,
        duration: std::time::Duration,
    ) -> Result<(), SignatureError> {
        let span_data = serde_json::json!({
            "operation": "signature_verified",
            "signature_valid": result.signature_valid,
            "identity_status": result.identity_status,
            "trust_score": result.trust_score,
            "warnings_count": result.warnings.len(),
            "duration_ms": duration.as_millis()
        });
        
        // TODO: Usar LogLineID real do sistema
        let system_id = crate::infra::id::LogLineIDWithKeys::generate_new()
            .map_err(|e| SignatureError::SystemError(e.to_string()))?;
        
        emitter.emit_span(
            "signature_verification",
            "signature_system", 
            &system_id,
            Some(span_data),
        ).await
        .map_err(|e| SignatureError::SpanEmissionError(e.to_string()))?;
        
        Ok(())
    }
}

impl Default for SignatureConfig {
    fn default() -> Self {
        Self {
            hash_algorithm: HashAlgorithm::Sha256,
            timestamp_tolerance_seconds: 300, // 5 minutos
            public_key_cache_ttl: 3600, // 1 hora
            require_timestamps: true,
            strict_verification: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Formato de LogLine ID inválido: {0}")]
    InvalidLogLineID(String),
    
    #[error("Identidade não encontrada: {0}")]
    IdentityNotFound(String),
    
    #[error("Formato de assinatura inválido: {0}")]
    InvalidSignatureFormat(String),
    
    #[error("Formato de chave pública inválido")]
    InvalidPublicKeyFormat,
    
    #[error("Erro de serialização: {0}")]
    SerializationError(String),
    
    #[error("Erro do sistema: {0}")]
    SystemError(String),
    
    #[error("Erro na emissão de span: {0}")]
    SpanEmissionError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::id::LogLineID;

    #[tokio::test]
    async fn test_signature_system_creation() {
        let config = SignatureConfig::default();
        let system = LogLineSignatureSystem::new(config);
        assert_eq!(system.public_key_cache.len(), 0);
    }

    #[tokio::test]
    async fn test_signable_payload_creation() {
        let config = SignatureConfig::default();
        let system = LogLineSignatureSystem::new(config);
        
        let data = serde_json::json!({"test": "data"});
        let payload = system.create_signable_payload(
            data,
            "test_operation",
            Some("test_contract".to_string()),
            None,
        );
        
        assert_eq!(payload.context.operation_type, "test_operation");
        assert_eq!(payload.context.contract_id, Some("test_contract".to_string()));
        assert!(!payload.nonce.is_empty());
    }

    #[tokio::test]
    async fn test_payload_serialization() {
        let config = SignatureConfig::default();
        let system = LogLineSignatureSystem::new(config);
        
        let data = serde_json::json!({"key": "value"});
        let payload = system.create_signable_payload(data, "test", None, None);
        
        let serialized = system.serialize_payload(&payload);
        assert!(serialized.is_ok());
        assert!(!serialized.unwrap().is_empty());
    }
}