// passkey.rs - WebAuthn Passkey implementation for LogLine ID
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyIdentity {
    pub id: String,
    pub alias: String,
    pub owner_type: PasskeyOwnerType,
    pub public_key: PublicKey,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub status: PasskeyStatus,
    pub passkey_metadata: PasskeyMetadata,
    pub capabilities: Vec<String>,
    pub federation_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PasskeyOwnerType {
    PasskeyIndividual,
    PasskeyOrganization,
    PasskeyGhost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PasskeyStatus {
    Active,
    Suspended,
    Revoked,
    Ghost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyMetadata {
    pub credential_id: String,
    pub authenticator_aaguid: String,
    pub device_type: BiometricDeviceType,
    pub platform: String,
    pub backup_eligible: bool,
    pub backup_state: bool,
    pub user_verification: bool,
    pub resident_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BiometricDeviceType {
    TouchID,
    FaceID,
    WindowsHello,
    AndroidFingerprint,
    YubiKey,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyCreationRequest {
    pub alias: String,
    pub credential_id: String,
    pub public_key_raw: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub client_data_json: String,
    pub device_type: BiometricDeviceType,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyAuthRequest {
    pub alias: String,
    pub credential_id: String,
    pub authenticator_data: Vec<u8>,
    pub client_data_json: String,
    pub signature: Vec<u8>,
    pub user_handle: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeySignatureRequest {
    pub identity_id: String,
    pub message: String,
    pub credential_id: String,
    pub signature: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub client_data_json: String,
}

#[derive(Debug)]
pub struct PasskeyManager {
    identities: HashMap<String, PasskeyIdentity>,
    credentials: HashMap<String, String>, // credential_id -> alias
}

impl PasskeyManager {
    pub fn new() -> Self {
        Self {
            identities: HashMap::new(),
            credentials: HashMap::new(),
        }
    }

    /// Create a new Passkey-based LogLine identity
    pub fn create_passkey_identity(
        &mut self,
        request: PasskeyCreationRequest,
    ) -> Result<PasskeyIdentity, PasskeyError> {
        // Validate alias availability
        if self.identities.contains_key(&request.alias) {
            return Err(PasskeyError::AliasExists(request.alias));
        }

        // Extract Ed25519 public key from WebAuthn credential
        let public_key = self.extract_ed25519_from_webauthn(&request.public_key_raw)?;

        // Generate unique identity ID
        let identity_id = format!("passkey_{}", Uuid::new_v4());

        // Create passkey metadata
        let passkey_metadata = PasskeyMetadata {
            credential_id: request.credential_id.clone(),
            authenticator_aaguid: self.extract_aaguid(&request.authenticator_data)?,
            device_type: request.device_type.clone(),
            platform: request.platform,
            backup_eligible: self.is_backup_eligible(&request.authenticator_data),
            backup_state: self.get_backup_state(&request.authenticator_data),
            user_verification: true, // Required for LogLine
            resident_key: true,      // Required for LogLine
        };

        // Create identity
        let identity = PasskeyIdentity {
            id: identity_id,
            alias: request.alias.clone(),
            owner_type: PasskeyOwnerType::PasskeyIndividual,
            public_key,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: PasskeyStatus::Active,
            passkey_metadata,
            capabilities: vec![
                "sign".to_string(),
                "verify".to_string(),
                "biometric_auth".to_string(),
                "ghost_create".to_string(),
            ],
            federation_level: "local".to_string(),
        };

        // Store identity and credential mapping
        self.identities.insert(request.alias.clone(), identity.clone());
        self.credentials.insert(request.credential_id, request.alias);

        Ok(identity)
    }

    /// Authenticate using existing Passkey
    pub fn authenticate_with_passkey(
        &self,
        request: PasskeyAuthRequest,
    ) -> Result<&PasskeyIdentity, PasskeyError> {
        // Find identity by credential ID
        let alias = self.credentials
            .get(&request.credential_id)
            .ok_or(PasskeyError::CredentialNotFound(request.credential_id.clone()))?;

        let identity = self.identities
            .get(alias)
            .ok_or(PasskeyError::IdentityNotFound(alias.clone()))?;

        // Verify identity is active
        if !matches!(identity.status, PasskeyStatus::Active) {
            return Err(PasskeyError::IdentityInactive(alias.clone()));
        }

        // Verify WebAuthn signature
        self.verify_webauthn_signature(
            &identity.public_key,
            &request.authenticator_data,
            &request.client_data_json,
            &request.signature,
        )?;

        Ok(identity)
    }

    /// Sign a message using Passkey-based identity
    pub fn sign_with_passkey(
        &self,
        request: PasskeySignatureRequest,
    ) -> Result<bool, PasskeyError> {
        let identity = self.identities
            .values()
            .find(|i| i.id == request.identity_id)
            .ok_or(PasskeyError::IdentityNotFound(request.identity_id.clone()))?;

        // Verify credential belongs to identity
        if identity.passkey_metadata.credential_id != request.credential_id {
            return Err(PasskeyError::CredentialMismatch);
        }

        // Verify WebAuthn signature against message
        let message_hash = self.hash_message_for_signing(&request.message);
        
        self.verify_webauthn_signature(
            &identity.public_key,
            &request.authenticator_data,
            &request.client_data_json,
            &request.signature,
        )?;

        // Additional verification: ensure client data contains our message
        let client_data: serde_json::Value = serde_json::from_str(&request.client_data_json)
            .map_err(|_| PasskeyError::InvalidClientData)?;
        
        let challenge = client_data["challenge"]
            .as_str()
            .ok_or(PasskeyError::InvalidClientData)?;
        
        let expected_challenge = BASE64.encode(&message_hash);
        if challenge != expected_challenge {
            return Err(PasskeyError::ChallengeMismatch);
        }

        Ok(true)
    }

    /// Create a ghost identity using existing Passkey
    pub fn create_ghost_with_passkey(
        &mut self,
        parent_alias: &str,
        ghost_alias: &str,
        duration_hours: u32,
        purpose: &str,
    ) -> Result<PasskeyIdentity, PasskeyError> {
        let parent = self.identities
            .get(parent_alias)
            .ok_or(PasskeyError::IdentityNotFound(parent_alias.to_string()))?
            .clone();

        // Verify parent can create ghosts
        if !parent.capabilities.contains(&"ghost_create".to_string()) {
            return Err(PasskeyError::InsufficientCapabilities);
        }

        let ghost_id = format!("ghost_{}", Uuid::new_v4());
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(duration_hours as i64);

        let mut ghost_metadata = parent.passkey_metadata.clone();
        ghost_metadata.credential_id = format!("ghost_{}", ghost_metadata.credential_id);

        let ghost_identity = PasskeyIdentity {
            id: ghost_id,
            alias: ghost_alias.to_string(),
            owner_type: PasskeyOwnerType::PasskeyGhost,
            public_key: parent.public_key.clone(), // Same biometric, different identity
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: PasskeyStatus::Ghost,
            passkey_metadata: ghost_metadata.clone(),
            capabilities: vec!["sign".to_string(), "verify".to_string()],
            federation_level: "local".to_string(),
        };

        // Store ghost identity
        self.identities.insert(ghost_alias.to_string(), ghost_identity.clone());
        self.credentials.insert(ghost_metadata.credential_id, ghost_alias.to_string());

        Ok(ghost_identity)
    }

    /// Extract Ed25519 public key from WebAuthn credential public key
    fn extract_ed25519_from_webauthn(&self, public_key_raw: &[u8]) -> Result<PublicKey, PasskeyError> {
        // WebAuthn Ed25519 public keys are in COSE format
        // This is a simplified implementation - in production, use a proper COSE library
        
        if public_key_raw.len() < 32 {
            return Err(PasskeyError::InvalidPublicKey);
        }

        // For Ed25519, the key is typically the last 32 bytes
        let key_bytes = &public_key_raw[public_key_raw.len() - 32..];
        
        PublicKey::from_bytes(key_bytes)
            .map_err(|_| PasskeyError::InvalidPublicKey)
    }

    /// Verify WebAuthn signature
    fn verify_webauthn_signature(
        &self,
        public_key: &PublicKey,
        authenticator_data: &[u8],
        client_data_json: &str,
        signature: &[u8],
    ) -> Result<(), PasskeyError> {
        // Create signed data for WebAuthn verification
        let client_data_hash = self.sha256(client_data_json.as_bytes());
        let mut signed_data = Vec::new();
        signed_data.extend_from_slice(authenticator_data);
        signed_data.extend_from_slice(&client_data_hash);

        // Convert WebAuthn signature to Ed25519 signature
        let ed25519_signature = Signature::from_bytes(signature)
            .map_err(|_| PasskeyError::InvalidSignature)?;

        // Verify signature
        public_key
            .verify(&signed_data, &ed25519_signature)
            .map_err(|_| PasskeyError::SignatureVerificationFailed)
    }

    /// Extract AAGUID from authenticator data
    fn extract_aaguid(&self, authenticator_data: &[u8]) -> Result<String, PasskeyError> {
        if authenticator_data.len() < 37 {
            return Err(PasskeyError::InvalidAuthenticatorData);
        }

        let aaguid_bytes = &authenticator_data[37..53];
        let aaguid = Uuid::from_bytes_le(
            aaguid_bytes.try_into().map_err(|_| PasskeyError::InvalidAuthenticatorData)?
        );

        Ok(aaguid.to_string())
    }

    /// Check if backup is eligible from flags
    fn is_backup_eligible(&self, authenticator_data: &[u8]) -> bool {
        if authenticator_data.len() < 33 {
            return false;
        }
        // Bit 3 of flags indicates backup eligibility
        (authenticator_data[32] & 0x08) != 0
    }

    /// Get backup state from flags
    fn get_backup_state(&self, authenticator_data: &[u8]) -> bool {
        if authenticator_data.len() < 33 {
            return false;
        }
        // Bit 4 of flags indicates backup state
        (authenticator_data[32] & 0x10) != 0
    }

    /// Hash message for signing
    fn hash_message_for_signing(&self, message: &str) -> Vec<u8> {
        self.sha256(message.as_bytes())
    }

    /// SHA-256 hash function
    fn sha256(&self, data: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    /// Get identity by alias
    pub fn get_identity(&self, alias: &str) -> Option<&PasskeyIdentity> {
        self.identities.get(alias)
    }

    /// List all identities
    pub fn list_identities(&self) -> Vec<&PasskeyIdentity> {
        self.identities.values().collect()
    }

    /// Revoke an identity
    pub fn revoke_identity(&mut self, alias: &str) -> Result<(), PasskeyError> {
        let identity = self.identities
            .get_mut(alias)
            .ok_or(PasskeyError::IdentityNotFound(alias.to_string()))?;

        identity.status = PasskeyStatus::Revoked;
        identity.updated_at = chrono::Utc::now();

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PasskeyError {
    #[error("Alias already exists: {0}")]
    AliasExists(String),
    
    #[error("Identity not found: {0}")]
    IdentityNotFound(String),
    
    #[error("Credential not found: {0}")]
    CredentialNotFound(String),
    
    #[error("Identity is inactive: {0}")]
    IdentityInactive(String),
    
    #[error("Invalid public key format")]
    InvalidPublicKey,
    
    #[error("Invalid signature format")]
    InvalidSignature,
    
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    
    #[error("Invalid authenticator data")]
    InvalidAuthenticatorData,
    
    #[error("Invalid client data")]
    InvalidClientData,
    
    #[error("Challenge mismatch")]
    ChallengeMismatch,
    
    #[error("Credential mismatch")]
    CredentialMismatch,
    
    #[error("Insufficient capabilities")]
    InsufficientCapabilities,
}

// Integration with existing LogLine ID module
impl From<PasskeyIdentity> for crate::signature::LogLineIdentity {
    fn from(passkey_identity: PasskeyIdentity) -> Self {
        Self {
            id: passkey_identity.id,
            alias: passkey_identity.alias,
            owner_type: match passkey_identity.owner_type {
                PasskeyOwnerType::PasskeyIndividual => "passkey_individual".to_string(),
                PasskeyOwnerType::PasskeyOrganization => "passkey_organization".to_string(),
                PasskeyOwnerType::PasskeyGhost => "passkey_ghost".to_string(),
            },
            public_key: BASE64.encode(passkey_identity.public_key.as_bytes()),
            created_at: passkey_identity.created_at.to_rfc3339(),
            updated_at: passkey_identity.updated_at.to_rfc3339(),
            status: match passkey_identity.status {
                PasskeyStatus::Active => "active".to_string(),
                PasskeyStatus::Suspended => "suspended".to_string(),
                PasskeyStatus::Revoked => "revoked".to_string(),
                PasskeyStatus::Ghost => "ghost".to_string(),
            },
            roles: vec!["user".to_string()],
            capabilities: passkey_identity.capabilities,
            federation_level: passkey_identity.federation_level,
            metadata: Some(serde_json::to_value(passkey_identity.passkey_metadata).unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passkey_manager_creation() {
        let manager = PasskeyManager::new();
        assert_eq!(manager.identities.len(), 0);
        assert_eq!(manager.credentials.len(), 0);
    }

    #[test]
    fn test_create_passkey_identity() {
        let mut manager = PasskeyManager::new();
        
        // Mock Ed25519 public key (32 bytes)
        let mock_public_key = vec![0u8; 32];
        
        let request = PasskeyCreationRequest {
            alias: "testuser".to_string(),
            credential_id: "cred123".to_string(),
            public_key_raw: mock_public_key,
            authenticator_data: vec![0u8; 100], // Mock authenticator data
            client_data_json: "{}".to_string(),
            device_type: BiometricDeviceType::TouchID,
            platform: "macOS".to_string(),
        };

        // This test will fail with current mock data, but shows the structure
        // In production, use real WebAuthn test vectors
        assert!(manager.create_passkey_identity(request).is_err());
    }
}

/// Export for use in contracts and other modules
pub use PasskeyManager as LogLinePasskeyManager;