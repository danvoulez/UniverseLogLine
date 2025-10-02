/// # Módulo de Lógica Computável LogLine ID
///
/// Este módulo contém toda a lógica computável para gerenciamento
/// de identidades LogLine, incluindo assinatura digital, identidades
/// ghost temporárias e federação entre nós.

pub mod signature;
pub mod ghost;
pub mod federation;
pub mod passkey;

// Re-exporta estruturas principais para facilitar o uso
pub use signature::{
    LogLineSignatureSystem,
    SignatureConfig,
    SignatureResult,
    VerificationResult,
    SignablePayload,
    SignatureContext,
    IdentityStatus,
    VerificationDetails,
    HashAlgorithm,
    SignatureError,
};

pub use ghost::{
    GhostIdentitySystem,
    GhostConfig,
    GhostIdentity,
    GhostStatus,
    GhostActivity,
    GhostActivityType,
    ClaimHints,
    GhostOperationResult,
    ClaimResult,
    GhostError,
};

pub use federation::{
    IdentityFederationSystem,
    FederationConfig,
    FederatedIdentity,
    FederatedNode,
    FederationStatus,
    NodeStatus,
    NodeEndorsement,
    EndorsementType,
    FederatedActivity,
    FederatedActivityType,
    FederationResult,
    FederationStats,
    FederationError,
};

/// Sistema unificado de gerenciamento de identidades LogLine
pub struct LogLineIdentityManager {
    /// Sistema de assinatura digital
    pub signature_system: LogLineSignatureSystem,
    
    /// Sistema de identidades ghost
    pub ghost_system: GhostIdentitySystem,
    
    /// Sistema de federação
    pub federation_system: IdentityFederationSystem,
}

impl LogLineIdentityManager {
    /// Cria novo gerenciador com configurações padrão
    pub fn new() -> Self {
        let signature_system = LogLineSignatureSystem::new(SignatureConfig::default());
        let ghost_system = GhostIdentitySystem::new(GhostConfig::default());
        let federation_system = IdentityFederationSystem::new(FederationConfig::default());
        
        Self {
            signature_system,
            ghost_system,
            federation_system,
        }
    }
    
    /// Cria gerenciador com configurações customizadas
    pub fn with_configs(
        signature_config: SignatureConfig,
        ghost_config: GhostConfig,
        federation_config: FederationConfig,
    ) -> Self {
        let signature_system = LogLineSignatureSystem::new(signature_config);
        let ghost_system = GhostIdentitySystem::new(ghost_config);
        let federation_system = IdentityFederationSystem::new(federation_config);
        
        Self {
            signature_system,
            ghost_system,
            federation_system,
        }
    }
    
    /// Configura emitente de spans para todos os sistemas
    pub fn with_span_emitter(self, span_emitter: crate::motor::span::SpanEmitter) -> Self {
        Self {
            signature_system: self.signature_system.with_span_emitter(span_emitter.clone()),
            ghost_system: self.ghost_system.with_span_emitter(span_emitter.clone()),
            federation_system: self.federation_system.with_span_emitter(span_emitter),
        }
    }
}

impl Default for LogLineIdentityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_manager_creation() {
        let manager = LogLineIdentityManager::new();
        // Verifica se todos os sistemas foram inicializados
        assert_eq!(manager.signature_system.config.hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(manager.ghost_system.config.default_lifetime_days, 30);
        assert_eq!(manager.federation_system.config.min_trust_threshold, 0.6);
    }

    #[test]
    fn test_identity_manager_with_custom_configs() {
        let signature_config = SignatureConfig {
            hash_algorithm: HashAlgorithm::Blake3,
            timestamp_tolerance_seconds: 600,
            ..Default::default()
        };
        
        let ghost_config = GhostConfig {
            default_lifetime_days: 7,
            max_ghosts_per_node: 5,
            ..Default::default()
        };
        
        let federation_config = FederationConfig {
            min_trust_threshold: 0.8,
            max_identities_per_node: 500,
            ..Default::default()
        };
        
        let manager = LogLineIdentityManager::with_configs(
            signature_config,
            ghost_config,
            federation_config,
        );
        
        assert_eq!(manager.signature_system.config.hash_algorithm, HashAlgorithm::Blake3);
        assert_eq!(manager.ghost_system.config.default_lifetime_days, 7);
        assert_eq!(manager.federation_system.config.min_trust_threshold, 0.8);
    }
}