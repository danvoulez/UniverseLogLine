//! Módulo de regras para o LogLine
//!
//! Este módulo define regras e ações de enforcement que podem ser
//! aplicadas aos spans na timeline.

/// Ação a ser tomada após a avaliação de regras de enforcement
#[derive(Debug, Clone)]
pub enum EnforcementAction {
    /// Permitir o span
    Allow,
    
    /// Rejeitar o span com uma razão
    Reject(String),
    
    /// Simular apenas (para modo offline)
    SimulateOnly,
}