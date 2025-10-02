use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use tokio::task::JoinHandle;
use crate::time::{AdaptiveClock, TimeModel};
use crate::grammar::LocalGrammar;

/// Gerencia múltiplos clocks simultâneos, um para cada projeto ativo
pub struct ProjectClockManager {
    /// Clocks ativos por projeto
    project_clocks: Arc<Mutex<HashMap<String, Arc<AdaptiveClock>>>>,
    /// Handles das tasks de clock
    clock_handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    /// Status de cada clock
    clock_status: Arc<Mutex<HashMap<String, ClockStatus>>>,
}

#[derive(Debug, Clone)]
pub struct ClockStatus {
    pub project_id: String,
    pub started_at: DateTime<Utc>,
    pub last_tick: DateTime<Utc>,
    pub tick_count: u64,
    pub is_active: bool,
    pub current_local_time: String,
}

impl ProjectClockManager {
    pub fn new() -> Self {
        Self {
            project_clocks: Arc::new(Mutex::new(HashMap::new())),
            clock_handles: Arc::new(Mutex::new(HashMap::new())),
            clock_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registra um novo clock para um projeto baseado na gramática
    pub async fn register_project_clock(
        &self, 
        project_id: String, 
        grammar: &LocalGrammar
    ) -> Result<(), ClockError> {
        let time_model = &grammar.time_model;
        let adaptive_clock = Arc::new(AdaptiveClock::new(time_model.clone()));
        
        // Registra o clock
        {
            let mut clocks = self.project_clocks.lock().unwrap();
            clocks.insert(project_id.clone(), adaptive_clock.clone());
        }
        
        // Inicia o status
        {
            let mut status = self.clock_status.lock().unwrap();
            status.insert(project_id.clone(), ClockStatus {
                project_id: project_id.clone(),
                started_at: Utc::now(),
                last_tick: Utc::now(),
                tick_count: 0,
                is_active: true,
                current_local_time: "initializing".to_string(),
            });
        }
        
        // Inicia a task do clock
        let handle = self.start_clock_task(project_id.clone(), adaptive_clock).await;
        
        {
            let mut handles = self.clock_handles.lock().unwrap();
            handles.insert(project_id, handle);
        }
        
        Ok(())
    }

    /// Inicia a task que gerencia o clock de um projeto
    async fn start_clock_task(
        &self,
        project_id: String,
        clock: Arc<AdaptiveClock>
    ) -> JoinHandle<()> {
        let status_ref = self.clock_status.clone();
        
        tokio::spawn(async move {
            let mut tick_count = 0u64;
            
            loop {
                // Espera pelo próximo tick baseado no modelo de tempo
                let next_tick_duration = clock.time_until_next_tick().await;
                tokio::time::sleep(next_tick_duration).await;
                
                // Verifica se ainda deve estar ativo
                let should_continue = {
                    let status = status_ref.lock().unwrap();
                    status.get(&project_id)
                        .map(|s| s.is_active)
                        .unwrap_or(false)
                };
                
                if !should_continue {
                    break;
                }
                
                // Executa o tick
                let local_time = clock.now_local().await;
                
                // Atualiza status
                {
                    let mut status = status_ref.lock().unwrap();
                    if let Some(project_status) = status.get_mut(&project_id) {
                        project_status.last_tick = Utc::now();
                        project_status.tick_count = tick_count;
                        project_status.current_local_time = local_time.to_string();
                    }
                }
                
                // Emite eventos de tick para listeners do projeto
                clock.emit_tick_event(&project_id, tick_count, &local_time).await;
                
                tick_count += 1;
                
                println!("🕐 Clock Tick - {}: {} (tick #{})", 
                    project_id, local_time, tick_count);
            }
            
            println!("⏹️ Clock parado para projeto: {}", project_id);
        })
    }

    /// Obtém o clock de um projeto específico
    pub fn get_project_clock(&self, project_id: &str) -> Option<Arc<AdaptiveClock>> {
        let clocks = self.project_clocks.lock().unwrap();
        clocks.get(project_id).cloned()
    }

    /// Obtém o status atual de um projeto
    pub fn get_project_status(&self, project_id: &str) -> Option<ClockStatus> {
        let status = self.clock_status.lock().unwrap();
        status.get(project_id).cloned()
    }

    /// Lista todos os projetos com clock ativo
    pub fn list_active_projects(&self) -> Vec<String> {
        let status = self.clock_status.lock().unwrap();
        status.values()
            .filter(|s| s.is_active)
            .map(|s| s.project_id.clone())
            .collect()
    }

    /// Para o clock de um projeto
    pub async fn stop_project_clock(&self, project_id: &str) -> Result<(), ClockError> {
        // Marca como inativo
        {
            let mut status = self.clock_status.lock().unwrap();
            if let Some(project_status) = status.get_mut(project_id) {
                project_status.is_active = false;
            }
        }
        
        // Para a task
        {
            let mut handles = self.clock_handles.lock().unwrap();
            if let Some(handle) = handles.remove(project_id) {
                handle.abort();
            }
        }
        
        // Remove das coleções
        {
            let mut clocks = self.project_clocks.lock().unwrap();
            clocks.remove(project_id);
        }
        
        {
            let mut status = self.clock_status.lock().unwrap();
            status.remove(project_id);
        }
        
        println!("🛑 Clock parado para projeto: {}", project_id);
        Ok(())
    }

    /// Para todos os clocks
    pub async fn stop_all_clocks(&self) {
        let project_ids: Vec<String> = {
            let status = self.clock_status.lock().unwrap();
            status.keys().cloned().collect()
        };
        
        for project_id in project_ids {
            let _ = self.stop_project_clock(&project_id).await;
        }
    }

    /// Estatísticas gerais dos clocks
    pub fn get_summary(&self) -> ClockManagerSummary {
        let status = self.clock_status.lock().unwrap();
        
        let active_count = status.values().filter(|s| s.is_active).count();
        let total_ticks: u64 = status.values().map(|s| s.tick_count).sum();
        
        ClockManagerSummary {
            total_projects: status.len(),
            active_projects: active_count,
            total_ticks_processed: total_ticks,
            uptime_since: status.values()
                .map(|s| s.started_at)
                .min()
                .unwrap_or_else(Utc::now),
        }
    }
}

#[derive(Debug)]
pub struct ClockManagerSummary {
    pub total_projects: usize,
    pub active_projects: usize,
    pub total_ticks_processed: u64,
    pub uptime_since: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClockError {
    #[error("Projeto já registrado: {0}")]
    ProjectAlreadyRegistered(String),
    #[error("Projeto não encontrado: {0}")]
    ProjectNotFound(String),
    #[error("Erro no modelo de tempo: {0}")]
    TimeModelError(String),
    #[error("Clock já parado para projeto: {0}")]
    ClockAlreadyStopped(String),
}

impl Default for ProjectClockManager {
    fn default() -> Self {
        Self::new()
    }
}