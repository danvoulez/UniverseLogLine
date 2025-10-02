//! Módulo de timeline para o LogLine
//!
//! Este módulo define estruturas e traits para trabalhar com a timeline,
//! que é a estrutura de dados central do LogLine.

use chrono::{DateTime, Utc};
use std::error::Error;
use serde::{Serialize, Deserialize};

/// Representa um span na timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Identificador único do span
    pub id: String,
    
    /// Canal ao qual o span pertence
    pub channel: String,
    
    /// Momento em que o span foi registrado
    pub timestamp: DateTime<Utc>,
    
    /// Dados do span em formato JSON
    pub data: Option<String>,
    
    /// Identificador do tenant (para multi-tenancy)
    pub tenant_id: Option<String>,
}

/// Trait para implementações de timeline
pub trait Timeline: Send + Sync {
    /// Adiciona um novo span à timeline
    fn append(&mut self, span: &Span) -> Result<(), Box<dyn Error>>;
    
    /// Consulta spans na timeline
    fn query(
        &self,
        channel: Option<&str>,
        time_range: Option<&TimeRange>,
        limit: Option<usize>,
    ) -> Result<Vec<Span>, Box<dyn Error>>;
    
    /// Obtém um span específico pelo ID
    fn get_by_id(&self, id: &str) -> Result<Option<Span>, Box<dyn Error>>;
}

/// Intervalo de tempo para consultas
#[derive(Debug, Clone)]
pub struct TimeRange {
    /// Início do intervalo (inclusivo)
    pub start: Option<DateTime<Utc>>,
    
    /// Fim do intervalo (inclusivo)
    pub end: Option<DateTime<Utc>>,
}