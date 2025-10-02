/// # LogLine Grammar Module
///
/// Sistema de gramáticas computáveis do LogLine.
/// Responsável por:
/// - Definir gramática universal (grammar_core)
/// - Carregar e validar gramáticas locais de projetos
/// - Gerenciar versionamento e compatibilidade
/// - Prover validação baseada em gramática
///
/// Estrutura:
/// - grammar_core.lll: Constituição universal imutável
/// - grammar_loader.rs: Carregador e validador de gramáticas
/// - grammar_validator.rs: Validador específico de regras

pub mod grammar_loader;
pub mod grammar_validator;

pub use grammar_loader::{
    GrammarLoader, 
    LoadedGrammar, 
    GrammarValidationResult, 
    GrammarRegistrationContract,
    GrammarLoaderConfig
};

pub use grammar_validator::{
    GrammarValidator,
    DetailedValidationResult,
    ValidationSeverity,
    RuleValidationResult,
    TimeModelValidationResult,
    WorkflowValidationResult
};