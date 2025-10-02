use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier, SecretKey};
use std::fs;
use std::path::Path;
use std::io::Error as IoError;

/// Key manager for LogLine ID crypto operations
pub struct KeyManager {
    signing_key: SigningKey,
}

impl KeyManager {
    /// Creates a new KeyManager with a new random key
    pub fn new() -> Self {
        use rand::rngs::OsRng;
        let mut csprng = OsRng;
        let secret_key = ed25519_dalek::SecretKey::generate(&mut csprng);
        let signing_key = SigningKey::from_bytes(&secret_key);
        Self { signing_key }
    }
    
    /// Creates a KeyManager from existing key bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let bytes_array: [u8; 32] = bytes.try_into()
            .map_err(|_| "Invalid signing key length".to_string())?;
        
        let signing_key = SigningKey::from_bytes(&bytes_array);
        Ok(Self { signing_key })
    }
    
    /// Get the public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
    
    /// Get the signing key bytes
    pub fn signing_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
    
    /// Sign data with the private key
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }
    
    /// Verify signature with a public key
    pub fn verify(public_key: &[u8], data: &[u8], signature: &Signature) -> Result<bool, String> {
        let public_key_bytes: [u8; 32] = public_key.try_into()
            .map_err(|_| "Invalid public key length".to_string())?;
        
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|_| "Invalid public key".to_string())?;
        
        Ok(verifying_key.verify(data, signature).is_ok())
    }
    
    /// Save key to a file in ~/.logline/{node_name}
    pub fn save_to_file(&self, node_name: &str) -> Result<(), IoError> {
        let home_dir = dirs::home_dir().ok_or(IoError::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found"
        ))?;
        
        let logline_dir = home_dir.join(".logline");
        std::fs::create_dir_all(&logline_dir)?;
        
        let file_path = logline_dir.join(node_name);
        
        let data = serde_json::json!({
            "secret_key": self.signing_key_bytes().to_vec()
        });
        
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(file_path, json)?;
        
        Ok(())
    }
    
    /// Load key from a file in ~/.logline/{node_name}
    pub fn load_from_file(node_name: &str) -> Result<Self, IoError> {
        let home_dir = dirs::home_dir().ok_or(IoError::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found"
        ))?;
        
        let file_path = home_dir.join(".logline").join(node_name);
        
        if !file_path.exists() {
            return Err(IoError::new(
                std::io::ErrorKind::NotFound,
                "Key file not found"
            ));
        }
        
        let json = fs::read_to_string(file_path)?;
        let data: serde_json::Value = serde_json::from_str(&json)
            .map_err(|e| IoError::new(std::io::ErrorKind::InvalidData, e))?;
        
        let secret_key = data["secret_key"].as_array()
            .ok_or(IoError::new(std::io::ErrorKind::InvalidData, "Invalid key format"))?
            .iter()
            .map(|v| v.as_u64().unwrap_or(0) as u8)
            .collect::<Vec<u8>>();
        
        Self::from_bytes(&secret_key)
            .map_err(|e| IoError::new(std::io::ErrorKind::InvalidData, e))
    }
    
    /// Get the raw signing key
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_generation() {
        let key_manager = KeyManager::new();
        assert_eq!(key_manager.public_key_bytes().len(), 32);
    }
    
    #[test]
    fn test_sign_and_verify() {
        let key_manager = KeyManager::new();
        let data = b"test message";
        
        let signature = key_manager.sign(data);
        let result = KeyManager::verify(&key_manager.public_key_bytes(), data, &signature);
        
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Test with wrong data
        let wrong_data = b"wrong message";
        let result = KeyManager::verify(&key_manager.public_key_bytes(), wrong_data, &signature);
        
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
    
    #[test]
    fn test_from_bytes() {
        let key_manager1 = KeyManager::new();
        let bytes = key_manager1.signing_key_bytes();
        
        let key_manager2 = KeyManager::from_bytes(&bytes).unwrap();
        
        assert_eq!(key_manager1.public_key_bytes(), key_manager2.public_key_bytes());
    }
}