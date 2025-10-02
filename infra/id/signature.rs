use ed25519_dalek::Signature;
use std::sync::{Arc, Mutex};

use crate::infra::id::logline_id::{LogLineID, LogLineKeyPair};

/// Service for handling LogLine ID signatures
pub struct SignatureService {
    current_id: Arc<Mutex<Option<LogLineKeyPair>>>,
}

impl SignatureService {
    /// Create a new signature service
    pub fn new() -> Self {
        Self {
            current_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize with an existing ID
    pub fn with_id(self, id: LogLineKeyPair) -> Self {
        *self.current_id.lock().unwrap() = Some(id);
        self
    }

    /// Get the current ID if available
    pub fn get_id(&self) -> Option<LogLineID> {
        self.current_id
            .lock()
            .unwrap()
            .as_ref()
            .map(|id| id.id.clone())
    }

    /// Sign data with the current ID  
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let guard = self.current_id.lock().unwrap();
        let keypair = guard
            .as_ref()
            .ok_or_else(|| "No LogLine ID configured".to_string())?;

        let signature = keypair.id.sign(&keypair.signing_key, data);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify data with a specific LogLine ID
    pub fn verify(&self, id: &LogLineID, data: &[u8], signature: &[u8]) -> Result<bool, String> {
        id.verify_signature(data, signature)
    }

    /// Verify data with the current ID
    pub fn verify_with_current(&self, data: &[u8], signature: &[u8]) -> Result<bool, String> {
        let guard = self.current_id.lock().unwrap();
        let keypair = guard
            .as_ref()
            .ok_or_else(|| "No LogLine ID configured".to_string())?;

        keypair.id.verify_signature(data, signature)
    }

    /// Generate a new ID
    pub fn generate_id(&self, node_name: &str) -> LogLineKeyPair {
        LogLineID::generate(node_name)
    }

    /// Set current ID
    pub fn set_id(&self, id: LogLineKeyPair) {
        *self.current_id.lock().unwrap() = Some(id);
    }

    /// Clear current ID
    pub fn clear_id(&self) {
        *self.current_id.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_service() {
        let service = SignatureService::new();
        let keypair = LogLineID::generate("test-node");

        // Set the ID
        service.set_id(keypair);

        // Test signing and verification
        let data = b"test message";
        let signature = service.sign(data).unwrap();

        assert!(service.verify_with_current(data, &signature).unwrap());

        // Test with wrong data
        let wrong_data = b"wrong message";
        assert!(!service.verify_with_current(wrong_data, &signature).unwrap());
    }

    #[test]
    fn test_verify_other_id() {
        let service = SignatureService::new();

        // Create two IDs
        let keypair1 = LogLineID::generate("node1");
        let keypair2 = LogLineID::generate("node2");

        // Set the first ID
        service.set_id(keypair1.clone());

        // Sign with the first ID
        let data = b"test message";
        let signature = service.sign(data).unwrap();

        // Verify with the second ID (should fail)
        assert!(!service
            .verify(&keypair2.id, data, &signature)
            .unwrap_or(false));

        // Verify with the first ID (should pass)
        assert!(service
            .verify(&keypair1.id, data, &signature)
            .unwrap_or(false));
    }
}
