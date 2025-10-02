//! LogLine ID Microservice Module
//!
//! This module provides a complete identity service for LogLine,
//! designed to work as an independent microservice.

pub mod logline_id;
pub mod signature;
pub mod serialization;
pub mod websocket;
pub mod key_manager;

// Re-export main types for external usage
pub use logline_id::{LogLineID, LogLineKeyPair, LogLineIDBuilder};
pub use signature::SignatureService;
pub use serialization::SerializationHelper;
pub use websocket::{IDWebSocketHandler, IDCommand, IDResponse};

// Service initialization functions
use std::sync::Arc;

/// Initialize the ID service
pub fn init_logline_id_service() -> Arc<SignatureService> {
    Arc::new(SignatureService::new())
}

/// Generate a new identity
pub fn generate_identity(node_name: &str) -> LogLineKeyPair {
    LogLineID::generate(node_name)
}

/// Create an identity service with existing identity
pub fn create_identity_service(identity: LogLineKeyPair) -> Arc<SignatureService> {
    Arc::new(SignatureService::new().with_id(identity))
}

/// Load identity from file and create service
pub fn load_identity_service(node_name: &str) -> Result<Arc<SignatureService>, std::io::Error> {
    match LogLineID::load_from_file(node_name) {
        Ok(keypair) => Ok(create_identity_service(keypair)),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::NotFound, e))
    }
}

/// Create WebSocket handler for the service
pub fn create_websocket_handler(service: Arc<SignatureService>) -> IDWebSocketHandler {
    IDWebSocketHandler::new(service)
}

/// Verify signature using LogLine ID
pub fn verify_signature(id: &LogLineID, data: &[u8], signature_base64: &str) -> Result<bool, String> {
    use base64::{decode_config, URL_SAFE_NO_PAD};
    
    let signature_bytes = decode_config(signature_base64, URL_SAFE_NO_PAD)
        .map_err(|e| format!("Invalid base64 signature: {}", e))?;
    
    id.verify_signature(data, &signature_bytes)
}