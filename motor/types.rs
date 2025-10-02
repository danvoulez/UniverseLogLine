use chrono::Utc;
use serde::{Serialize, Deserialize};

/// Estado do motor de execução
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineStatus {
    /// Motor pronto para processar spans
    Ready,
    
    /// Motor processando um span
    Processing,
    
    /// Motor em modo de erro
    Error,
    
    /// Motor em modo de pausa
    Paused,
    
    /// Motor desligado
    Shutdown,
}

/// Modos de execução do motor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Execução normal com aplicação de todas as regras
    Normal,
    
    /// Execução em modo de simulação (não persiste mudanças)
    Simulation,
    
    /// Execução em modo de emergência (bypass de algumas regras)
    Emergency,
    
    /// Execução em modo de auditoria (logs extras)
    Audit,
}

/// Erros possíveis durante o processamento
#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    /// Erro de validação de dados
    #[error("Erro de validação: {0}")]
    Validation(String),
    
    /// Erro na execução do span
    #[error("Erro na execução: {0}")]
    Execution(String),
    
    /// Erro de persistência
    #[error("Erro de persistência: {0}")]
    Persistence(String),
    
    /// Span rejeitado pelas regras
    #[error("Rejeitado pelas regras: {0}")]
    RulesRejected(String),
    
    /// Erro na aplicação de rollback
    #[error("Erro no rollback: {0}")]
    RollbackFailed(String),
    
    /// Erro de sistema
    #[error("Erro interno: {0}")]
    System(String),
}