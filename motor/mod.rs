//! Motor de execução do LogLine
//!
//! Este módulo contém os componentes do motor de execução
//! responsáveis pelo processamento de spans e execução de contratos.

mod engine;
mod executor;
mod rollback;
mod rollback_test;
mod rotator;
mod runtime;
mod scheduler;
mod span;
mod timekeeper;
mod types;

// Re-exports públicos
pub use engine::Engine;
pub use executor::{Executor, ExecutionContext, ExecutionResult};
pub use rollback::{RollbackManager, RollbackResult};
pub use rotator::Rotator;
pub use runtime::Runtime;
pub use scheduler::{Scheduler, ScheduledTask};
pub use span::Span;
pub use timekeeper::TimeKeeper;
pub use types::{EngineStatus, ExecutionMode, ProcessingError};