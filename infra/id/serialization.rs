use serde::{Serialize, Deserialize};
use std::path::Path;
use std::fs;
use std::io::{Error as IoError, ErrorKind};

use crate::infra::id::logline_id::{LogLineID, LogLineKeyPair};

/// Helper for serializing/deserializing LogLine IDs
pub struct SerializationHelper;

impl SerializationHelper {
    /// Serialize an ID to JSON
    pub fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
        serde_json::to_string_pretty(value)
            .map_err(|e| format!("Serialization error: {}", e))
    }
    
    /// Deserialize an ID from JSON
    pub fn from_json<T: for<'de> Deserialize<'de>>(json: &str) -> Result<T, String> {
        serde_json::from_str(json)
            .map_err(|e| format!("Deserialization error: {}", e))
    }
    
    /// Save an ID to a specific file path
    pub fn save_to_file<T: Serialize>(value: &T, path: &Path) -> Result<(), IoError> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| IoError::new(ErrorKind::InvalidData, e))?;
            
        fs::write(path, json)?;
        Ok(())
    }
    
    /// Load an ID from a specific file path
    pub fn load_from_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, IoError> {
        if !path.exists() {
            return Err(IoError::new(ErrorKind::NotFound, "File not found"));
        }
        
        let json = fs::read_to_string(path)?;
        let value = serde_json::from_str(&json)
            .map_err(|e| IoError::new(ErrorKind::InvalidData, e))?;
            
        Ok(value)
    }
    
    /// Save an ID to the standard location (~/.logline/{node_name})
    pub fn save_id_to_default_location(keypair: &LogLineKeyPair) -> Result<(), IoError> {
        let home_dir = dirs::home_dir().ok_or(IoError::new(
            ErrorKind::NotFound,
            "Home directory not found"
        ))?;
        
        let logline_dir = home_dir.join(".logline");
        fs::create_dir_all(&logline_dir)?;
        
        let file_path = logline_dir.join(&keypair.id.node_name);
        
        Self::save_to_file(keypair, &file_path)
    }
    
    /// Load an ID from the standard location (~/.logline/{node_name})
    pub fn load_id_from_default_location(node_name: &str) -> Result<LogLineKeyPair, IoError> {
        let home_dir = dirs::home_dir().ok_or(IoError::new(
            ErrorKind::NotFound,
            "Home directory not found"
        ))?;
        
        let file_path = home_dir.join(".logline").join(node_name);
        
        Self::load_from_file(&file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_serialize_deserialize_json() {
        let keypair = LogLineID::generate("test-serialize");
        
        let json = SerializationHelper::to_json(&keypair).unwrap();
        let deserialized: LogLineKeyPair = SerializationHelper::from_json(&json).unwrap();
        
        assert_eq!(keypair.id.node_name, deserialized.id.node_name);
        assert_eq!(keypair.id.id, deserialized.id.id);
    }
    
    #[test]
    fn test_save_load_file() {
        let keypair = LogLineID::generate("test-file");
        
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test-id.json");
        
        // Save to file
        SerializationHelper::save_to_file(&keypair, &file_path).unwrap();
        
        // Load from file
        let loaded: LogLineKeyPair = SerializationHelper::load_from_file(&file_path).unwrap();
        
        assert_eq!(keypair.id.node_name, loaded.id.node_name);
        assert_eq!(keypair.id.id, loaded.id.id);
    }
}