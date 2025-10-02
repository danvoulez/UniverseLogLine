// simple_auth.rs - Simple Ed25519-first authentication with optional enhancements
use ed25519_dalek::{PublicKey, SecretKey, Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LogLine ID: Ed25519 first, everything else is optional
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLineIdentity {
    // CORE REQUIREMENTS (always present)
    pub id: String,
    pub alias: String,
    pub ed25519_public_key: PublicKey,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    
    // OPTIONAL ENHANCEMENTS (can be added later)
    pub authentication_methods: Vec<AuthMethod>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Authentication methods that can be attached to an identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// WebAuthn Passkey for humans
    Passkey {
        credential_id: String,
        device_type: String,
        backup_eligible: bool,
    },
    
    /// SSH public key (like GitHub)
    SshKey {
        key_type: String, // "ssh-ed25519", "ssh-rsa", etc.
        public_key: String,
        comment: Option<String>,
    },
    
    /// Additional Ed25519 keys for different purposes
    AdditionalEd25519 {
        purpose: String, // "signing", "encryption", "backup"
        public_key: PublicKey,
    },
    
    /// External service verification
    ExternalService {
        service: String, // "github", "twitter", "email"
        identifier: String,
        verified: bool,
    },
}

/// Simple authentication request
#[derive(Debug, Clone)]
pub struct AuthRequest {
    pub alias: String,
    pub message: String,
    pub signature: Signature,
    pub method: Option<String>, // Which method to use if multiple
}

/// Simple authentication manager
pub struct SimpleAuthManager {
    identities: HashMap<String, LogLineIdentity>,
}

impl SimpleAuthManager {
    pub fn new() -> Self {
        Self {
            identities: HashMap::new(),
        }
    }

    /// Create identity with Ed25519 key (required) + optional methods
    pub fn create_identity(
        &mut self,
        alias: String,
        ed25519_public_key: PublicKey,
        optional_methods: Vec<AuthMethod>,
    ) -> Result<LogLineIdentity, SimpleAuthError> {
        
        if self.identities.contains_key(&alias) {
            return Err(SimpleAuthError::AliasExists(alias));
        }

        let identity = LogLineIdentity {
            id: format!("logline_{}", uuid::Uuid::new_v4()),
            alias: alias.clone(),
            ed25519_public_key,
            created_at: chrono::Utc::now(),
            status: "active".to_string(),
            authentication_methods: optional_methods,
            metadata: HashMap::new(),
        };

        self.identities.insert(alias, identity.clone());
        Ok(identity)
    }

    /// Authenticate using Ed25519 signature (always works)
    pub fn authenticate(&self, request: AuthRequest) -> Result<&LogLineIdentity, SimpleAuthError> {
        let identity = self.identities
            .get(&request.alias)
            .ok_or(SimpleAuthError::IdentityNotFound(request.alias.clone()))?;

        // Always verify with Ed25519 public key
        let message_bytes = request.message.as_bytes();
        
        identity.ed25519_public_key
            .verify(message_bytes, &request.signature)
            .map_err(|_| SimpleAuthError::InvalidSignature)?;

        Ok(identity)
    }

    /// Add authentication method to existing identity
    pub fn add_auth_method(
        &mut self,
        alias: &str,
        method: AuthMethod,
        ed25519_signature: Signature, // Prove you own the identity
    ) -> Result<(), SimpleAuthError> {
        
        let identity = self.identities
            .get_mut(alias)
            .ok_or(SimpleAuthError::IdentityNotFound(alias.to_string()))?;

        // Verify signature with existing Ed25519 key
        let message = format!("add_auth_method:{}", serde_json::to_string(&method).unwrap());
        identity.ed25519_public_key
            .verify(message.as_bytes(), &ed25519_signature)
            .map_err(|_| SimpleAuthError::InvalidSignature)?;

        // Add the method
        identity.authentication_methods.push(method);
        
        Ok(())
    }

    /// Get identity by alias
    pub fn get_identity(&self, alias: &str) -> Option<&LogLineIdentity> {
        self.identities.get(alias)
    }

    /// Check if identity has specific auth method
    pub fn has_auth_method(&self, alias: &str, method_type: &str) -> bool {
        if let Some(identity) = self.identities.get(alias) {
            identity.authentication_methods.iter().any(|method| {
                match (method, method_type) {
                    (AuthMethod::Passkey { .. }, "passkey") => true,
                    (AuthMethod::SshKey { .. }, "ssh") => true,
                    (AuthMethod::AdditionalEd25519 { .. }, "ed25519") => true,
                    (AuthMethod::ExternalService { service, .. }, s) => service == s,
                    _ => false,
                }
            })
        } else {
            false
        }
    }

    /// Create ghost identity (temporary)
    pub fn create_ghost(
        &mut self,
        parent_alias: &str,
        ghost_alias: String,
        duration_hours: u32,
        ed25519_signature: Signature,
    ) -> Result<LogLineIdentity, SimpleAuthError> {
        
        let parent = self.identities
            .get(parent_alias)
            .ok_or(SimpleAuthError::IdentityNotFound(parent_alias.to_string()))?
            .clone();

        // Verify parent signature
        let message = format!("create_ghost:{}:{}", ghost_alias, duration_hours);
        parent.ed25519_public_key
            .verify(message.as_bytes(), &ed25519_signature)
            .map_err(|_| SimpleAuthError::InvalidSignature)?;

        // Create ghost with same Ed25519 key but different alias
        let mut ghost_identity = LogLineIdentity {
            id: format!("ghost_{}", uuid::Uuid::new_v4()),
            alias: ghost_alias.clone(),
            ed25519_public_key: parent.ed25519_public_key.clone(), // Same key!
            created_at: chrono::Utc::now(),
            status: "ghost".to_string(),
            authentication_methods: parent.authentication_methods.clone(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("parent_alias".to_string(), serde_json::Value::String(parent_alias.to_string()));
                meta.insert("expires_at".to_string(), serde_json::Value::String(
                    (chrono::Utc::now() + chrono::Duration::hours(duration_hours as i64)).to_rfc3339()
                ));
                meta
            },
        };

        self.identities.insert(ghost_alias, ghost_identity.clone());
        Ok(ghost_identity)
    }
}

/// Helper functions for common scenarios
impl SimpleAuthManager {
    /// Create identity from SSH public key (like GitHub)
    pub fn create_from_ssh_key(
        &mut self,
        alias: String,
        ssh_public_key: String,
        ed25519_private_key: SecretKey, // User provides this
    ) -> Result<LogLineIdentity, SimpleAuthError> {
        
        let ed25519_public_key = PublicKey::from(&ed25519_private_key);
        
        let ssh_method = AuthMethod::SshKey {
            key_type: extract_ssh_key_type(&ssh_public_key),
            public_key: ssh_public_key,
            comment: None,
        };

        self.create_identity(alias, ed25519_public_key, vec![ssh_method])
    }

    /// Create identity with Passkey
    pub fn create_from_passkey(
        &mut self,
        alias: String,
        passkey_credential_id: String,
        device_type: String,
        ed25519_private_key: SecretKey, // Derived from Passkey
    ) -> Result<LogLineIdentity, SimpleAuthError> {
        
        let ed25519_public_key = PublicKey::from(&ed25519_private_key);
        
        let passkey_method = AuthMethod::Passkey {
            credential_id: passkey_credential_id,
            device_type,
            backup_eligible: false,
        };

        self.create_identity(alias, ed25519_public_key, vec![passkey_method])
    }

    /// Create identity with GitHub integration
    pub fn create_from_github(
        &mut self,
        alias: String,
        github_username: String,
        ed25519_private_key: SecretKey,
    ) -> Result<LogLineIdentity, SimpleAuthError> {
        
        let ed25519_public_key = PublicKey::from(&ed25519_private_key);
        
        let github_method = AuthMethod::ExternalService {
            service: "github".to_string(),
            identifier: github_username,
            verified: false, // Will be verified later
        };

        self.create_identity(alias, ed25519_public_key, vec![github_method])
    }
}

fn extract_ssh_key_type(ssh_key: &str) -> String {
    ssh_key.split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum SimpleAuthError {
    #[error("Alias already exists: {0}")]
    AliasExists(String),
    
    #[error("Identity not found: {0}")]
    IdentityNotFound(String),
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;

    #[test]
    fn test_simple_identity_creation() {
        let mut manager = SimpleAuthManager::new();
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);

        let identity = manager.create_identity(
            "testuser".to_string(),
            keypair.public,
            vec![]
        ).unwrap();

        assert_eq!(identity.alias, "testuser");
        assert_eq!(identity.ed25519_public_key, keypair.public);
    }

    #[test]
    fn test_authentication() {
        let mut manager = SimpleAuthManager::new();
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);

        // Create identity
        manager.create_identity(
            "testuser".to_string(),
            keypair.public,
            vec![]
        ).unwrap();

        // Sign message
        let message = "test message";
        let signature = keypair.sign(message.as_bytes());

        // Authenticate
        let auth_request = AuthRequest {
            alias: "testuser".to_string(),
            message: message.to_string(),
            signature,
            method: None,
        };

        let result = manager.authenticate(auth_request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_auth_method() {
        let mut manager = SimpleAuthManager::new();
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);

        // Create identity
        manager.create_identity(
            "testuser".to_string(),
            keypair.public,
            vec![]
        ).unwrap();

        // Add SSH key method
        let ssh_method = AuthMethod::SshKey {
            key_type: "ssh-ed25519".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5...".to_string(),
            comment: Some("test@example.com".to_string()),
        };

        let message = format!("add_auth_method:{}", serde_json::to_string(&ssh_method).unwrap());
        let signature = keypair.sign(message.as_bytes());

        let result = manager.add_auth_method("testuser", ssh_method, signature);
        assert!(result.is_ok());
        assert!(manager.has_auth_method("testuser", "ssh"));
    }
}

/// Export for integration with existing LogLine ID system
pub use SimpleAuthManager as LogLineSimpleAuthManager;