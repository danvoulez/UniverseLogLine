use std::fmt;
use std::error::Error;
use std::io;

#[derive(Debug)]
pub enum LogLineError {
    // Erros de I/O
    IoError(io::Error),
    
    // Erros de serialização/deserialização
    SerializationError(String),
    DeserializationError(String),
    
    // Erros de spans
    SpanValidationError(String),
    SpanNotFound(String),
    InvalidSpanId(String),
    
    // Erros de contrato
    ContractValidationError(String),
    InvalidContractState(String),
    ProhibitedTransition(String),
    
    // Erros de regras e lógica
    RuleViolation(String),
    LogicEvaluationError(String),
    
    // Erros de assinatura e criptografia
    SignatureVerificationFailed,
    KeyGenerationError,
    
    // Erros da Timeline
    TimelineError(String),
    
    // Outros erros
    NotImplemented,
    GeneralError(String),
}

// Implementar conversões de erro comuns
impl From<io::Error> for LogLineError {
    fn from(err: io::Error) -> Self {
        LogLineError::IoError(err)
    }
}

impl From<serde_json::Error> for LogLineError {
    fn from(err: serde_json::Error) -> Self {
        LogLineError::DeserializationError(err.to_string())
    }
}

// Implementar Display para LogLineError
impl fmt::Display for LogLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLineError::IoError(err) => write!(f, "Erro de I/O: {}", err),
            LogLineError::SerializationError(msg) => write!(f, "Erro de serialização: {}", msg),
            LogLineError::DeserializationError(msg) => write!(f, "Erro de deserialização: {}", msg),
            LogLineError::SpanValidationError(msg) => write!(f, "Erro de validação de span: {}", msg),
            LogLineError::SpanNotFound(id) => write!(f, "Span não encontrado: {}", id),
            LogLineError::InvalidSpanId(id) => write!(f, "ID de span inválido: {}", id),
            LogLineError::ContractValidationError(msg) => write!(f, "Erro de validação de contrato: {}", msg),
            LogLineError::InvalidContractState(msg) => write!(f, "Estado de contrato inválido: {}", msg),
            LogLineError::ProhibitedTransition(msg) => write!(f, "Transição proibida: {}", msg),
            LogLineError::RuleViolation(msg) => write!(f, "Violação de regra: {}", msg),
            LogLineError::LogicEvaluationError(msg) => write!(f, "Erro de avaliação lógica: {}", msg),
            LogLineError::SignatureVerificationFailed => write!(f, "Verificação de assinatura falhou"),
            LogLineError::KeyGenerationError => write!(f, "Erro na geração de chaves"),
            LogLineError::TimelineError(msg) => write!(f, "Erro na timeline: {}", msg),
            LogLineError::NotImplemented => write!(f, "Funcionalidade não implementada"),
            LogLineError::GeneralError(msg) => write!(f, "Erro geral: {}", msg),
        }
    }
}

// Implementar Error para LogLineError
impl Error for LogLineError {}