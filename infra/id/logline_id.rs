//! LogLine ID - Core Identity Service
//! 
//! This module provides the core identity system for LogLine,
//! designed to work as an independent microservice.

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signer, Signature};
use rand::rngs::OsRng;
use serde::{Serialize, Deserialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use base64::{encode_config, decode_config, URL_SAFE_NO_PAD};

/// Core LogLine Identity Structure
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LogLineID {
    /// Unique identifier for the identity
    pub id: Uuid,
    
    /// Node name (human-readable identifier)
    pub node_name: String,
    
    /// Base64 encoded public key
    pub public_key: String,
    
    /// Optional alias for the identity
    pub alias: Option<String>,
    
    /// Tenant scope for multi-tenancy
    pub tenant_id: Option<String>,
    
    /// Organization flag
    pub is_org: bool,
    
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
    
    /// Creation timestamp
    pub issued_at: DateTime<Utc>,
}

/// Par de chaves LogLine que inclui a chave privada e o ID
#[derive(Clone, Serialize, Deserialize)]
pub struct LogLineKeyPair {
    /// Keypair do ed25519
    keypair: Keypair,
    
    /// O LogLine ID associado
    pub id: LogLineID,
}

impl LogLineID {
    /// Creates a new LogLine ID from a public key and node name
    pub fn new(node_name: &str, public_key: &PublicKey, alias: Option<String>, tenant_id: Option<String>, is_org: bool) -> Self {
        let pk_bytes = public_key.as_bytes();
        let pk_encoded = encode_config(pk_bytes, URL_SAFE_NO_PAD);
        
        Self {
            id: Uuid::new_v4(),
            node_name: node_name.to_string(),
            public_key: pk_encoded,
            alias,
            tenant_id,
            is_org,
            metadata: None,
            issued_at: Utc::now(),
        }
    }
    
    /// Define um alias para o ID
    pub fn with_alias(mut self, alias: &str) -> Self {
        self.alias = Some(alias.to_string());
        self
    }
    
    /// Define o tenant ID
    pub fn with_tenant(mut self, tenant_id: &str) -> Self {
        self.tenant_id = Some(tenant_id.to_string());
        self
    }
    
    /// Define se é um ID organizacional
    pub fn with_is_org(mut self, is_org: bool) -> Self {
        self.is_org = is_org;
        self
    }
    
    /// Adiciona metadados ao ID
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
    
    /// Obtém a chave pública
    pub fn get_public_key(&self) -> Result<PublicKey, String> {
        let pk_bytes = decode_config(&self.public_key, URL_SAFE_NO_PAD)
            .map_err(|e| format!("Erro ao decodificar a chave pública: {}", e))?;
            
        PublicKey::from_bytes(&pk_bytes)
            .map_err(|e| format!("Chave pública inválida: {}", e))
    }
    
    /// Verifica uma assinatura com esta chave pública
    pub fn verify_signature(&self, message: &[u8], signature: &[u8]) -> Result<bool, String> {
        use ed25519_dalek::Verifier;
        
        let public_key = self.get_public_key()?;
        
        // Converter assinatura para o formato ed25519
        if signature.len() != 64 {
            return Err("Tamanho de assinatura inválido".to_string());
        }
        
        let signature_array: [u8; 64] = signature.try_into()
            .map_err(|_| "Erro ao converter assinatura".to_string())?;
        
        let signature = ed25519_dalek::Signature::from_bytes(&signature_array);
            
        Ok(public_key.verify(message, &signature).is_ok())
    }
    
    /// Serializa o LogLine ID para JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    
    /// Formato amigável para exibição
    pub fn display_name(&self) -> String {
        if let Some(alias) = &self.alias {
            if self.is_org {
                format!("Organização: {}", alias)
            } else {
                alias.clone()
            }
        } else {
            let short_key = if self.public_key.len() > 16 {
                format!("{}...", &self.public_key[..16])
            } else {
                self.public_key.clone()
            };
            
            if self.is_org {
                format!("Org: {}", short_key)
            } else {
                format!("ID: {}", short_key)
            }
        }
    }
}

impl fmt::Display for LogLineID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl FromStr for LogLineID {
    type Err = serde_json::Error;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl LogLineID {
    /// Carrega o LogLine ID a partir de uma string
    pub fn from_string(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
    
    /// Salva o LogLine ID em um arquivo no diretório ~/.logline
    pub fn save_to_file(&self, signing_key: &[u8]) -> Result<(), String> {
        let home_dir = dirs::home_dir().ok_or("Não foi possível obter o diretório home")?;
        let logline_dir = home_dir.join(".logline");
        
        // Criar diretório ~/.logline se não existir
        if !logline_dir.exists() {
            std::fs::create_dir_all(&logline_dir)
                .map_err(|e| format!("Erro ao criar diretório ~/.logline: {}", e))?;
        }
        
        let file_name = self.alias.as_deref().unwrap_or(&self.node_name);
        let file_path = logline_dir.join(file_name);
        
        // Serializar o ID
        let json = self.to_json()
            .map_err(|e| format!("Erro ao serializar ID: {}", e))?;
            
        // Criar um objeto com ID e chave de assinatura
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), serde_json::from_str(&json).unwrap());
        obj.insert("signing_key".to_string(), serde_json::Value::String(encode_config(signing_key, URL_SAFE_NO_PAD)));
        
        // Salvar no arquivo
        let json_with_key = serde_json::to_string_pretty(&obj)
            .map_err(|e| format!("Erro ao serializar: {}", e))?;
            
        std::fs::write(&file_path, json_with_key)
            .map_err(|e| format!("Erro ao escrever arquivo {}: {}", file_path.display(), e))?;
            
        Ok(())
    }
    
    /// Carrega um LogLine ID e chave de assinatura a partir de um arquivo
    pub fn load_from_file(alias: &str) -> Result<LogLineKeyPair, String> {
        let home_dir = dirs::home_dir().ok_or("Não foi possível obter o diretório home")?;
        let file_path = home_dir.join(".logline").join(alias);
        
        // Verificar se o arquivo existe
        if !file_path.exists() {
            return Err(format!("Arquivo não encontrado: {}", file_path.display()));
        }
        
        // Ler o arquivo
        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Erro ao ler arquivo {}: {}", file_path.display(), e))?;
            
        // Deserializar
        let data: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Erro ao deserializar: {}", e))?;
            
        // Extrair ID
        let id_value = data.get("id")
            .ok_or("ID não encontrado no arquivo")?;
            
        let id: LogLineID = serde_json::from_value(id_value.clone())
            .map_err(|e| format!("Erro ao deserializar ID: {}", e))?;
            
        // Extrair chave de assinatura
        let key_str = data.get("signing_key")
            .and_then(|v| v.as_str())
            .ok_or("Chave de assinatura não encontrada")?;
            
        let key_bytes = decode_config(key_str, URL_SAFE_NO_PAD)
            .map_err(|e| format!("Erro ao decodificar chave: {}", e))?;
            
        // Create the key pair
        LogLineKeyPair::from_secret_key(
            &id.node_name,
            &key_bytes, 
            id.alias.clone(), 
            id.tenant_id.clone(), 
            id.is_org
        )
    }
}

impl LogLineKeyPair {
    /// Generates a new LogLine key pair with node name
    pub fn generate(
        node_name: &str,
        alias: Option<String>,
        tenant_id: Option<String>,
        is_org: bool
    ) -> Self {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        
        let id = LogLineID::new(node_name, &keypair.public, alias, tenant_id, is_org);
        
        Self { keypair, id }
    }
    
    /// Creates a key pair from existing secret key
    pub fn from_secret_key(
        node_name: &str,
        secret_key_bytes: &[u8],
        alias: Option<String>,
        tenant_id: Option<String>,
        is_org: bool
    ) -> Result<Self, String> {
        if secret_key_bytes.len() != 32 {
            return Err("Tamanho inválido para chave secreta".to_string());
        }
        
        // Criar chave secreta
        let secret_array: [u8; 32] = secret_key_bytes.try_into()
            .map_err(|_| "Tamanho inválido para chave secreta".to_string())?;
        let secret = SecretKey::from(secret_array);
            
        // Derivar chave pública
        let public = PublicKey::from(&secret);
        
        // Criar keypair
        let keypair = Keypair {
            secret,
            public,
        };
        
        // Create LogLine ID
        let id = LogLineID::new(node_name, &public, alias, tenant_id, is_org);
        
        Ok(Self { keypair, id })
    }
    
    /// Assina uma mensagem com a chave privada
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        let signature = self.keypair.sign(message);
        signature.to_bytes().to_vec()
    }
    
    /// Obtém a chave secreta como bytes
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.keypair.secret.to_bytes()
    }
    
    /// Obtém a chave pública como bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.keypair.public.to_bytes()
    }
    
    /// Serializa a chave secreta de forma segura, com senha
    pub fn export_secret_key(&self, password: &str) -> Result<String, String> {
        // Implementar exportação segura com criptografia
        // Aqui apenas um exemplo simples, em produção usar criptografia adequada
        let key_bytes = self.keypair.secret.to_bytes();
        let encoded = encode_config(&key_bytes, URL_SAFE_NO_PAD);
        
        // Em produção, usar algo como: 
        // let encrypted = encrypt_aes_gcm(key_bytes, password);
        
        Ok(encoded)
    }
    
    /// Imports a password-protected secret key
    pub fn import_secret_key(
        node_name: &str,
        encoded: &str, 
        password: &str,
        alias: Option<String>,
        tenant_id: Option<String>,
        is_org: bool
    ) -> Result<Self, String> {
        // Decode the key
        let key_bytes = decode_config(encoded, URL_SAFE_NO_PAD)
            .map_err(|e| format!("Erro ao decodificar a chave: {}", e))?;
            
        // In production: let decrypted = decrypt_aes_gcm(encoded, password);
        
        Self::from_secret_key(node_name, &key_bytes, alias, tenant_id, is_org)
    }
}

/// Facilita criação de LogLine IDs e KeyPairs
pub struct LogLineIDBuilder;

impl LogLineID {
    /// Helper method to create a new LogLine ID with keys
    pub fn generate(node_name: &str) -> LogLineKeyPair {
        LogLineIDBuilder::new_user(node_name, Some(node_name.to_string()), None)
    }
}

impl LogLineIDBuilder {
    /// Creates a new ID for a user
    pub fn new_user(node_name: &str, alias: Option<String>, tenant_id: Option<String>) -> LogLineKeyPair {
        LogLineKeyPair::generate(node_name, alias, tenant_id, false)
    }
    
    /// Creates a new ID for an organization
    pub fn new_organization(node_name: &str, alias: Option<String>, tenant_id: Option<String>) -> LogLineKeyPair {
        LogLineKeyPair::generate(node_name, alias, tenant_id, true)
    }
    
    /// Creates a multi-tenant ID for the system itself
    pub fn new_system(node_name: &str) -> LogLineKeyPair {
        let keypair = LogLineKeyPair::generate(node_name, Some("LogLine System".to_string()), None, true);
        
        // Add system-specific metadata
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "type".to_string(), 
            serde_json::Value::String("system".to_string())
        );
        metadata.insert(
            "permissions".to_string(), 
            serde_json::Value::String("all".to_string())
        );
        
        LogLineKeyPair {
            keypair: keypair.keypair,
            id: keypair.id.with_metadata(serde_json::Value::Object(metadata)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_keypair() {
        let keypair = LogLineKeyPair::generate("test-node", Some("Test User".to_string()), Some("tenant1".to_string()), false);
        assert_eq!(keypair.id.node_name, "test-node");
        assert_eq!(keypair.id.alias, Some("Test User".to_string()));
        assert_eq!(keypair.id.tenant_id, Some("tenant1".to_string()));
        assert_eq!(keypair.id.is_org, false);
    }
    
    #[test]
    fn test_sign_and_verify() {
        let keypair = LogLineKeyPair::generate("test-node", None, None, false);
        let message = b"Hello, LogLine!";
        
        // Assinar mensagem
        let signature = keypair.sign(message);
        
        // Verificar assinatura
        let result = keypair.id.verify_signature(message, &signature);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Verificar assinatura com mensagem errada
        let wrong_message = b"Wrong message!";
        let result = keypair.id.verify_signature(wrong_message, &signature);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
    
    #[test]
    fn test_serialization() {
        let keypair = LogLineIDBuilder::new_user("test-node", Some("Usuário de Teste".to_string()), Some("tenant99".to_string()));
        
        // Serialize to JSON
        let json = keypair.id.to_json().unwrap();
        
        // Deserialize back
        let deserialized: LogLineID = serde_json::from_str(&json).unwrap();
        
        assert_eq!(keypair.id.public_key, deserialized.public_key);
        assert_eq!(keypair.id.alias, deserialized.alias);
        assert_eq!(keypair.id.tenant_id, deserialized.tenant_id);
        assert_eq!(keypair.id.is_org, deserialized.is_org);
        assert_eq!(keypair.id.node_name, deserialized.node_name);
    }
    
    #[test]
    fn test_display_name() {
        let id1 = LogLineIDBuilder::new_user("alice-node", Some("Alice".to_string()), None).id;
        let id2 = LogLineIDBuilder::new_organization("acme-node", Some("ACME Corp".to_string()), None).id;
        let id3 = LogLineIDBuilder::new_user("anon-node", None, None).id;
        
        assert_eq!(id1.display_name(), "Alice");
        assert_eq!(id2.display_name(), "Organização: ACME Corp");
        assert!(id3.display_name().starts_with("ID: "));
    }
}