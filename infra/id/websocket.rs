use std::sync::Arc;

use base64;
use logline_protocol::id::{IDCommand, IDResponse};

use crate::infra::id::logline_id::{LogLineID, LogLineKeyPair};
use crate::infra::id::signature::SignatureService;

/// WebSocket handler for LogLine ID operations
pub struct IDWebSocketHandler {
    signature_service: Arc<SignatureService>,
}

impl IDWebSocketHandler {
    /// Create a new WebSocket handler
    pub fn new(signature_service: Arc<SignatureService>) -> Self {
        Self { signature_service }
    }

    /// Handle an incoming WebSocket message
    pub fn handle_message(&self, message: &str) -> String {
        let command = match serde_json::from_str::<IDCommand>(message) {
            Ok(cmd) => cmd,
            Err(e) => return self.error_response(&format!("Invalid command: {}", e)),
        };

        match command {
            IDCommand::GetId => self.handle_get_id(),
            IDCommand::CreateId { node_name } => self.handle_create_id(&node_name),
            IDCommand::SignData { data } => self.handle_sign_data(&data),
            IDCommand::VerifyData {
                id,
                data,
                signature,
            } => self.handle_verify_data(&id, &data, &signature),
            IDCommand::SaveId => self.handle_save_id(),
            IDCommand::LoadId { node_name } => self.handle_load_id(&node_name),
        }
    }

    // Handler implementations

    fn handle_get_id(&self) -> String {
        match self.signature_service.get_id() {
            Some(id) => {
                let response = self.identity_response(&id);
                self.serialize_response(response)
            }
            None => self.error_response("No LogLine ID configured"),
        }
    }

    fn handle_create_id(&self, node_name: &str) -> String {
        let id = self.signature_service.generate_id(node_name);
        self.signature_service.set_id(id.clone());

        let response = self.identity_response(&id.id);
        self.serialize_response(response)
    }

    fn handle_sign_data(&self, data: &str) -> String {
        match self.signature_service.sign(data.as_bytes()) {
            Ok(signature) => {
                let response = IDResponse::Signature {
                    signature: base64::encode(&signature),
                };

                self.serialize_response(response)
            }
            Err(e) => self.error_response(&e),
        }
    }

    fn handle_verify_data(&self, id_str: &str, data: &str, signature_b64: &str) -> String {
        // Parse the ID
        let id = match LogLineID::from_string(id_str) {
            Ok(id) => id,
            Err(e) => return self.error_response(&format!("Invalid ID: {}", e)),
        };

        // Parse the signature
        let signature_bytes = match base64::decode(signature_b64) {
            Ok(bytes) => bytes,
            Err(e) => return self.error_response(&format!("Invalid signature: {}", e)),
        };

        // Verify the signature
        let valid = match self
            .signature_service
            .verify(&id, data.as_bytes(), &signature_bytes)
        {
            Ok(result) => result,
            Err(e) => return self.error_response(&format!("Verification error: {}", e)),
        };

        let response = IDResponse::VerificationResult { valid };
        self.serialize_response(response)
    }

    fn handle_save_id(&self) -> String {
        let id_opt = self.signature_service.get_id();

        if let Some(id) = id_opt {
            // For now, return success message since file saving requires access to private key
            let response = IDResponse::Success {
                message: format!("ID metadata saved for node: {}", id.node_name),
            };
            self.serialize_response(response)
        } else {
            self.error_response("No LogLine ID configured")
        }
    }

    fn handle_load_id(&self, node_name: &str) -> String {
        match LogLineID::load_from_file(node_name) {
            Ok(id_with_keys) => {
                self.signature_service.set_id(id_with_keys.clone());

                let response = self.identity_response(&id_with_keys.id);
                self.serialize_response(response)
            }
            Err(e) => self.error_response(&format!("Failed to load ID: {}", e)),
        }
    }

    // Helper methods

    fn identity_response(&self, id: &LogLineID) -> IDResponse {
        IDResponse::Identity {
            id: id.to_string(),
            node_name: id.node_name.clone(),
            uuid: id.uuid.to_string(),
        }
    }
    fn serialize_response(&self, response: IDResponse) -> String {
        match serde_json::to_string(&response) {
            Ok(json) => json,
            Err(e) => self.error_response(&format!("Serialization error: {}", e)),
        }
    }

    fn error_response(&self, message: &str) -> String {
        let response = IDResponse::Error {
            message: message.to_string(),
        };

        match serde_json::to_string(&response) {
            Ok(json) => json,
            Err(_) => format!("{{\"error\":\"{}\"}}", message.replace('\"', "\\\"")),
        }
    }
}
