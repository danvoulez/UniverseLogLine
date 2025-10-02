use crate::motor::span::Span;
use crate::motor::executor::{Executor, ExecutionResult};
use crate::motor::types::{EngineStatus, ExecutionMode, ProcessingError};
use crate::enforcement::Enforcer;
use std::sync::{Arc, Mutex};
use chrono::Utc;

/// Motor de processamento de spans
pub struct Engine {
    /// Status atual do motor
    status: Arc<Mutex<EngineStatus>>,
    
    /// Executor para spans
    executor: Executor,
    
    /// Sistema de enforcer
    enforcer: Arc<dyn Enforcer + Send + Sync>,
    
    /// Modo de execução
    mode: ExecutionMode,
}

impl Engine {
    /// Cria uma nova instância do motor
    pub fn new(
        executor: Executor, 
        enforcer: Arc<dyn Enforcer + Send + Sync>,
        mode: ExecutionMode
    ) -> Self {
        Self {
            status: Arc::new(Mutex::new(EngineStatus::Ready)),
            executor,
            enforcer,
            mode,
        }
    }
    
    /// Processa um span
    pub fn process_span(&self, span: &mut Span) -> Result<ExecutionResult, ProcessingError> {
        // Atualizar status
        {
            let mut status = self.status.lock().unwrap();
            *status = EngineStatus::Processing;
        }
        
        // Verificar se o modo é simulação
        let simulate_only = self.mode == ExecutionMode::Simulation;
        
        // Aplicar regras pelo enforcer
        let enforcement_result = self.enforcer.enforce(span)?;
        
        // Se rejeitado pelo enforcer, retornar erro
        if !enforcement_result.is_allowed() {
            let mut status = self.status.lock().unwrap();
            *status = EngineStatus::Error;
            
            return Err(ProcessingError::RulesRejected(
                format!("Span rejeitado pelo enforcer: {}", enforcement_result.reason())
            ));
        }
        
        // Executar o span
        let execution_result = if simulate_only || enforcement_result.is_simulate_only() {
            // Modo simulação - não executa realmente
            ExecutionResult {
                success: true,
                execution_time: Utc::now(),
                output: serde_json::json!({
                    "simulated": true,
                    "span_id": span.id.to_string(),
                }),
                changes: vec![],
            }
        } else {
            // Execução real
            self.executor.execute(span)?
        };
        
        // Marcar span como processado
        if execution_result.success && !simulate_only {
            span.mark_processed();
        }
        
        // Atualizar status
        {
            let mut status = self.status.lock().unwrap();
            *status = EngineStatus::Ready;
        }
        
        Ok(execution_result)
    }
    
    /// Obtém o status atual do motor
    pub fn status(&self) -> EngineStatus {
        let status = self.status.lock().unwrap();
        *status
    }
    
    /// Altera o modo de execução
    pub fn set_mode(&mut self, mode: ExecutionMode) {
        self.mode = mode;
    }
    
    /// Reseta o motor para estado inicial
    pub fn reset(&self) {
        let mut status = self.status.lock().unwrap();
        *status = EngineStatus::Ready;
    }
}