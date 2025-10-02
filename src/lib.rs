//! LogLine: Sistema de registro distribuído para instituições computacionais
//!
//! LogLine é um sistema de registro distribuído que utiliza spans como unidade
//! fundamental de realidade, projetado para implementar instituições computacionais.
//!
//! # Arquitetura
//!
//! O sistema é dividido em vários módulos:
//!
//! * `cli`: Interface de linha de comando para interagir com o sistema
//! * `enforcement`: Sistema de aplicação de regras e políticas
//! * `federation`: Federação entre nós LogLine
//! * `grammar`: Gramáticas para validação de spans
//! * `infra`: Componentes de infraestrutura como identidades
//! * `motor`: Motor de execução e processamento de spans
//! * `rules`: Regras de negócio e validação
//! * `time`: Gerenciamento de tempo e clocks
//! * `timeline`: Armazenamento e manipulação de timelines

// Exporta módulos
pub mod cli;
pub mod enforcement;
pub mod federation;
pub mod grammar;
pub mod infra;
pub mod motor;
pub mod rules;
pub mod time;
pub mod timeline;

// Re-exporta símbolos comumente utilizados
pub use motor::span::Span;
pub use timeline::Timeline;
pub use enforcement::enforcer::Enforcer;
pub use infra::id::{LogLineID, LogLineKeyPair, LogLineIDBuilder};

/// Versão do protocolo LogLine
pub const LOGLINE_PROTOCOL_VERSION: &str = "0.1.0";

/// Verificação de versão do protocolo
pub fn verify_protocol_compatibility(version: &str) -> bool {
    // Na versão atual, apenas verifica se as versões principais são iguais
    version.split('.').next() == LOGLINE_PROTOCOL_VERSION.split('.').next()
}