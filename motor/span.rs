//! Definição e implementação do tipo Span
//!
//! Span é a unidade fundamental de processamento no sistema LogLine.

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use crate::infra::id::LogLineID;
use std::collections::HashMap;
use uuid::Uuid;

/// Um span representa uma unidade de processamento na timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Identificador único do span
    pub id: Uuid,
    
    /// Timestamp de criação do span
    pub timestamp: DateTime<Utc>,
    
    /// LogLine ID do criador do span
    pub logline_id: String,
    
    /// Tipo de span (contrato, transação, etc)
    pub tipo: String,
    
    /// Título descritivo do span
    pub title: String,
    
    /// Dados específicos do span como JSON
    pub data: serde_json::Value,
    
    /// ID do tenant ao qual este span pertence (multi-tenant)
    pub tenant_id: Option<String>,
    
    /// Metadados adicionais
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    
    /// Assinatura digital do span (opcional)
    pub signature: Option<String>,
    
    /// Vetor de hashes de spans relacionados
    #[serde(default)]
    pub related_spans: Vec<String>,
    
    /// Indica se este span já foi processado
    #[serde(default)]
    pub processed: bool,
    
    /// Tags para classificação e busca
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Span {
    /// Cria um novo span
    pub fn new(tipo: &str, title: &str, logline_id: &str, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            logline_id: logline_id.to_string(),
            tipo: tipo.to_string(),
            title: title.to_string(),
            data,
            tenant_id: None,
            metadata: HashMap::new(),
            signature: None,
            related_spans: Vec::new(),
            processed: false,
            tags: Vec::new(),
        }
    }
    
    /// Cria um novo span com tenant_id (multi-tenant)
    pub fn new_with_tenant(
        tipo: &str, 
        title: &str, 
        logline_id: &str, 
        data: serde_json::Value, 
        tenant_id: &str
    ) -> Self {
        let mut span = Self::new(tipo, title, logline_id, data);
        span.tenant_id = Some(tenant_id.to_string());
        span
    }

    /// Adiciona uma tag ao span
    pub fn add_tag(&mut self, tag: &str) {
        if !self.tags.contains(&tag.to_string()) {
            self.tags.push(tag.to_string());
        }
    }
    
    /// Adiciona uma assinatura digital ao span
    pub fn sign(&mut self, signature: &str) {
        self.signature = Some(signature.to_string());
    }
    
    /// Relaciona este span a outro span
    pub fn relate_to(&mut self, span_hash: &str) {
        if !self.related_spans.contains(&span_hash.to_string()) {
            self.related_spans.push(span_hash.to_string());
        }
    }
    
    /// Adiciona um par chave-valor aos metadados
    pub fn add_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }
    
    /// Marca o span como processado
    pub fn mark_processed(&mut self) {
        self.processed = true;
    }
    
    /// Verifica se o span possui uma determinada tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_string())
    }
    
    /// Calcula o hash do span para referência
    pub fn hash(&self) -> String {
        use sha2::{Sha256, Digest};
        
        let span_json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(span_json.as_bytes());
        let result = hasher.finalize();
        
        format!("{:x}", result)
    }
}