/// # LogLine Time Module - Sistema de Tempo Computável
///
/// Módulo responsável pelo tempo declarativo baseado em gramática.
/// Substitui o conceito de "tick fixo" por modelos temporais
/// específicos de cada domínio/projeto.
///
/// Componentes:
/// - time_model.rs: Sistema adaptativo de tempo por gramática
/// - adaptive_clock.rs: Clock que usa modelo declarado
///
/// Cada projeto pode declarar seu próprio modelo de tempo:
/// - Dias úteis (minicontratos)
/// - Slots de 30min (TV)
/// - Ciclos de experimento (Lab)
/// - Microsegundos (sistema)

pub mod time_model;
pub mod adaptive_clock;
pub mod project_clock_manager;
pub mod multi_project_example;

pub use time_model::{AdaptiveTimeSystem, TimeState, TemporalClockStatus, CalculationResult, TemporalEvent};
pub use adaptive_clock::AdaptiveClock;
pub use project_clock_manager::{ProjectClockManager, ClockStatus, ClockManagerSummary};
pub use multi_project_example::MultiProjectClockSystem;