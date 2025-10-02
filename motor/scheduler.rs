/// # LogLine Runtime Engine - Scheduler
///
/// Sistema de agendamento e fila computável baseada em ticks.
/// Responsável por:
/// - Gerenciar jobs com scheduled_tick, traj_budget, fallback_flow
/// - Coordenar execução da fila do Lab e curadoria da TV
/// - Controlar prazos, timeouts e recursos computacionais
/// - Implementar prioridades e balanceamento de carga
/// - Garantir execução ordenada e auditável
///
/// O Scheduler é fundamental para:
/// - Fila de jobs do LogLine Lab
/// - Agendamento de curadoria da VoulezVous.TV
/// - Execução de contratos .lll com prazo
/// - Orquestração de agentes computacionais

use std::collections::{HashMap, BinaryHeap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::motor::timekeeper::{Timekeeper, TickListener, TRAJ_DURATION_MICROS};
use crate::motor::span::SpanEmitter;
use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};

/// Prioridade de execução
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
    Emergency = 4,
}

/// Status de um job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Scheduled,
    Running,
    Completed,
    Failed,
    Timeout,
    Cancelled,
}

/// Tipos de job suportados
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobType {
    /// Execução de contrato .lll
    Contract { contract_path: String },
    /// Job do LogLine Lab (transcode, analysis, etc.)
    Lab { 
        operation: String,
        input_path: String,
        output_path: String,
    },
    /// Curadoria da TV (download, edit, upload)
    TvCuration {
        operation: String, // "download", "edit", "upload", "schedule"
        video_id: String,
        target_slot: Option<u64>,
    },
    /// Execução de agente
    Agent {
        agent_id: String,
        command: String,
        context: serde_json::Value,
    },
    /// Job customizado
    Custom {
        handler: String,
        payload: serde_json::Value,
    },
}

/// Definição de um job agendado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub job_id: Uuid,
    pub job_type: JobType,
    pub priority: Priority,
    pub scheduled_tick: u64,
    pub traj_budget: u64,
    pub timeout_ticks: Option<u64>,
    pub retry_count: u8,
    pub max_retries: u8,
    pub fallback_flow: Option<String>,
    pub dependencies: Vec<Uuid>, // Jobs que devem ser executados antes
    pub tags: Vec<String>,
    pub created_by: LogLineID,
    pub created_at: u64,
    pub metadata: serde_json::Value,
}

/// Estado de execução de um job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecution {
    pub job_id: Uuid,
    pub status: JobStatus,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub traj_used: u64,
    pub error_message: Option<String>,
    pub result: Option<serde_json::Value>,
    pub span_id: Option<String>,
}

/// Job em execução (para heap de prioridade)
#[derive(Debug, Clone)]
struct PendingJob {
    job: ScheduledJob,
    priority_score: u64, // Combinação de prioridade + urgência temporal
}

impl PartialEq for PendingJob {
    fn eq(&self, other: &Self) -> bool {
        self.priority_score == other.priority_score
    }
}

impl Eq for PendingJob {}

impl PartialOrd for PendingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Heap reversa - maior prioridade primeiro
        other.priority_score.cmp(&self.priority_score)
    }
}

/// Resultado de execução de job
#[derive(Debug)]
pub struct JobResult {
    pub job_id: Uuid,
    pub success: bool,
    pub traj_used: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Estatísticas do scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub total_jobs_scheduled: u64,
    pub total_jobs_completed: u64,
    pub total_jobs_failed: u64,
    pub jobs_in_queue: usize,
    pub jobs_running: usize,
    pub total_trajs_used: u64,
    pub average_job_duration_trajs: f64,
}

/// Motor de agendamento principal
pub struct Scheduler {
    id_with_keys: LogLineIDWithKeys,
    span_emitter: Arc<SpanEmitter>,
    
    // Fila de jobs pendentes (heap de prioridade)
    pending_jobs: Arc<Mutex<BinaryHeap<PendingJob>>>,
    
    // Jobs em execução
    running_jobs: Arc<RwLock<HashMap<Uuid, JobExecution>>>,
    
    // Histórico de execuções
    job_history: Arc<RwLock<HashMap<Uuid, JobExecution>>>,
    
    // Canal para comunicação com executores
    job_sender: Arc<Mutex<Option<mpsc::UnboundedSender<ScheduledJob>>>>,
    job_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<ScheduledJob>>>>,
    
    // Canal para resultados
    result_sender: Arc<Mutex<Option<mpsc::UnboundedSender<JobResult>>>>,
    result_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<JobResult>>>>,
    
    // Configurações
    max_concurrent_jobs: usize,
    default_traj_budget: u64,
    default_timeout_ticks: u64,
    
    // Estatísticas
    stats: Arc<RwLock<SchedulerStats>>,
}

impl Scheduler {
    /// Cria nova instância do Scheduler
    pub fn new(
        id_with_keys: LogLineIDWithKeys,
        span_emitter: Arc<SpanEmitter>,
    ) -> Self {
        let (job_sender, job_receiver) = mpsc::unbounded_channel();
        let (result_sender, result_receiver) = mpsc::unbounded_channel();

        Self {
            id_with_keys,
            span_emitter,
            pending_jobs: Arc::new(Mutex::new(BinaryHeap::new())),
            running_jobs: Arc::new(RwLock::new(HashMap::new())),
            job_history: Arc::new(RwLock::new(HashMap::new())),
            job_sender: Arc::new(Mutex::new(Some(job_sender))),
            job_receiver: Arc::new(Mutex::new(Some(job_receiver))),
            result_sender: Arc::new(Mutex::new(Some(result_sender))),
            result_receiver: Arc::new(Mutex::new(Some(result_receiver))),
            max_concurrent_jobs: 4, // Configurável
            default_traj_budget: 10000, // ~640ms
            default_timeout_ticks: 100000, // ~6.4s
            stats: Arc::new(RwLock::new(SchedulerStats {
                total_jobs_scheduled: 0,
                total_jobs_completed: 0,
                total_jobs_failed: 0,
                jobs_in_queue: 0,
                jobs_running: 0,
                total_trajs_used: 0,
                average_job_duration_trajs: 0.0,
            })),
        }
    }

    /// Agenda um novo job
    pub fn schedule_job(&self, mut job: ScheduledJob) -> Result<Uuid, Box<dyn std::error::Error>> {
        // Define valores padrão se não especificados
        if job.traj_budget == 0 {
            job.traj_budget = self.default_traj_budget;
        }
        
        if job.timeout_ticks.is_none() {
            job.timeout_ticks = Some(self.default_timeout_ticks);
        }

        let job_id = job.job_id;

        // Calcula score de prioridade
        let priority_score = self.calculate_priority_score(&job);
        
        let pending_job = PendingJob {
            job,
            priority_score,
        };

        // Adiciona à fila
        {
            let mut pending = self.pending_jobs.lock().unwrap();
            pending.push(pending_job);
        }

        // Atualiza estatísticas
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_jobs_scheduled += 1;
            stats.jobs_in_queue += 1;
        }

        Ok(job_id)
    }

    /// Agenda job do Lab
    pub fn schedule_lab_job(
        &self,
        operation: &str,
        input_path: &str,
        output_path: &str,
        traj_budget: Option<u64>,
        scheduled_tick: Option<u64>,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        let current_tick = self.current_tick();
        
        let job = ScheduledJob {
            job_id: Uuid::new_v4(),
            job_type: JobType::Lab {
                operation: operation.to_string(),
                input_path: input_path.to_string(),
                output_path: output_path.to_string(),
            },
            priority: Priority::Normal,
            scheduled_tick: scheduled_tick.unwrap_or(current_tick),
            traj_budget: traj_budget.unwrap_or(self.default_traj_budget),
            timeout_ticks: Some(self.default_timeout_ticks),
            retry_count: 0,
            max_retries: 3,
            fallback_flow: None,
            dependencies: Vec::new(),
            tags: vec!["lab".to_string(), operation.to_string()],
            created_by: self.id_with_keys.id.clone(),
            created_at: current_tick,
            metadata: serde_json::json!({
                "operation": operation,
                "input": input_path,
                "output": output_path
            }),
        };

        self.schedule_job(job)
    }

    /// Agenda job de curadoria da TV
    pub fn schedule_tv_job(
        &self,
        operation: &str,
        video_id: &str,
        target_slot: Option<u64>,
        priority: Priority,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        let current_tick = self.current_tick();
        
        let job = ScheduledJob {
            job_id: Uuid::new_v4(),
            job_type: JobType::TvCuration {
                operation: operation.to_string(),
                video_id: video_id.to_string(),
                target_slot,
            },
            priority,
            scheduled_tick: target_slot.unwrap_or(current_tick),
            traj_budget: match operation {
                "download" => 20000, // ~1.28s
                "edit" => 100000,    // ~6.4s
                "upload" => 50000,   // ~3.2s
                "schedule" => 1000,  // ~64ms
                _ => self.default_traj_budget,
            },
            timeout_ticks: Some(self.default_timeout_ticks * 5), // Timeout maior para TV
            retry_count: 0,
            max_retries: 2,
            fallback_flow: Some("tv_fallback".to_string()),
            dependencies: Vec::new(),
            tags: vec!["tv".to_string(), operation.to_string()],
            created_by: self.id_with_keys.id.clone(),
            created_at: current_tick,
            metadata: serde_json::json!({
                "operation": operation,
                "video_id": video_id,
                "target_slot": target_slot
            }),
        };

        self.schedule_job(job)
    }

    /// Cancela um job
    pub fn cancel_job(&self, job_id: &Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Remove da fila pendente
        {
            let mut pending = self.pending_jobs.lock().unwrap();
            let mut temp_vec = Vec::new();
            
            while let Some(pending_job) = pending.pop() {
                if pending_job.job.job_id != *job_id {
                    temp_vec.push(pending_job);
                }
            }
            
            for job in temp_vec {
                pending.push(job);
            }
        }

        // Marca como cancelado se estiver em execução
        {
            let mut running = self.running_jobs.write().unwrap();
            if let Some(execution) = running.get_mut(job_id) {
                execution.status = JobStatus::Cancelled;
                execution.completed_at = Some(self.current_tick());
            }
        }

        Ok(())
    }

    /// Inicia o scheduler
    pub async fn start(&self, timekeeper: Arc<Timekeeper>) -> Result<(), Box<dyn std::error::Error>> {
        // Registra como listener do timekeeper
        let scheduler_listener = SchedulerTickListener::new(Arc::new(self.clone()));
        timekeeper.add_listener(Arc::new(scheduler_listener));

        // Inicia processador de jobs
        self.start_job_processor().await?;

        // Inicia processador de resultados
        self.start_result_processor().await?;

        Ok(())
    }

    /// Processa jobs na fila
    async fn start_job_processor(&self) -> Result<(), Box<dyn std::error::Error>> {
        let scheduler = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut receiver = {
                let mut receiver_opt = scheduler.job_receiver.lock().unwrap();
                receiver_opt.take().unwrap()
            };

            while let Some(job) = receiver.recv().await {
                if let Err(e) = scheduler.execute_job(job).await {
                    eprintln!("Erro executando job: {:?}", e);
                }
            }
        });

        Ok(())
    }

    /// Processa resultados de jobs
    async fn start_result_processor(&self) -> Result<(), Box<dyn std::error::Error>> {
        let scheduler = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut receiver = {
                let mut receiver_opt = scheduler.result_receiver.lock().unwrap();
                receiver_opt.take().unwrap()
            };

            while let Some(result) = receiver.recv().await {
                if let Err(e) = scheduler.handle_job_result(result).await {
                    eprintln!("Erro processando resultado: {:?}", e);
                }
            }
        });

        Ok(())
    }

    /// Executa um job
    async fn execute_job(&self, job: ScheduledJob) -> Result<(), Box<dyn std::error::Error>> {
        let job_id = job.job_id;
        let start_tick = self.current_tick();

        // Registra início da execução
        {
            let mut running = self.running_jobs.write().unwrap();
            running.insert(job_id, JobExecution {
                job_id,
                status: JobStatus::Running,
                started_at: Some(start_tick),
                completed_at: None,
                traj_used: 0,
                error_message: None,
                result: None,
                span_id: None,
            });
        }

        // Atualiza estatísticas
        {
            let mut stats = self.stats.write().unwrap();
            stats.jobs_running += 1;
            stats.jobs_in_queue = stats.jobs_in_queue.saturating_sub(1);
        }

        // Emite span de início
        self.emit_job_start_span(&job).await?;

        // Executa baseado no tipo
        let result = match &job.job_type {
            JobType::Lab { operation, input_path, output_path } => {
                self.execute_lab_job(job_id, operation, input_path, output_path).await
            },
            JobType::TvCuration { operation, video_id, target_slot } => {
                self.execute_tv_job(job_id, operation, video_id, *target_slot).await
            },
            JobType::Contract { contract_path } => {
                self.execute_contract_job(job_id, contract_path).await
            },
            JobType::Agent { agent_id, command, context } => {
                self.execute_agent_job(job_id, agent_id, command, context).await
            },
            JobType::Custom { handler, payload } => {
                self.execute_custom_job(job_id, handler, payload).await
            },
        };

        // Envia resultado
        if let Some(result_sender) = self.result_sender.lock().unwrap().as_ref() {
            let job_result = match result {
                Ok(result_data) => JobResult {
                    job_id,
                    success: true,
                    traj_used: self.current_tick() - start_tick,
                    result: result_data,
                    error: None,
                },
                Err(e) => JobResult {
                    job_id,
                    success: false,
                    traj_used: self.current_tick() - start_tick,
                    result: None,
                    error: Some(e.to_string()),
                },
            };

            let _ = result_sender.send(job_result);
        }

        Ok(())
    }

    /// Executa job do Lab
    async fn execute_lab_job(
        &self, 
        job_id: Uuid, 
        operation: &str, 
        input_path: &str, 
        output_path: &str
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        // Simulação de execução do Lab
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(Some(serde_json::json!({
            "job_id": job_id,
            "operation": operation,
            "input": input_path,
            "output": output_path,
            "status": "completed"
        })))
    }

    /// Executa job da TV
    async fn execute_tv_job(
        &self,
        job_id: Uuid,
        operation: &str,
        video_id: &str,
        target_slot: Option<u64>
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        // Simulação de execução da TV
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        Ok(Some(serde_json::json!({
            "job_id": job_id,
            "operation": operation,
            "video_id": video_id,
            "target_slot": target_slot,
            "status": "completed"
        })))
    }

    /// Executa job de contrato
    async fn execute_contract_job(
        &self,
        job_id: Uuid,
        contract_path: &str
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        // Integração com executor de contratos
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        Ok(Some(serde_json::json!({
            "job_id": job_id,
            "contract": contract_path,
            "status": "executed"
        })))
    }

    /// Executa job de agente
    async fn execute_agent_job(
        &self,
        job_id: Uuid,
        agent_id: &str,
        command: &str,
        context: &serde_json::Value
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        // Integração com runtime de agentes
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        
        Ok(Some(serde_json::json!({
            "job_id": job_id,
            "agent_id": agent_id,
            "command": command,
            "context": context,
            "status": "completed"
        })))
    }

    /// Executa job customizado
    async fn execute_custom_job(
        &self,
        job_id: Uuid,
        handler: &str,
        payload: &serde_json::Value
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        // Handler customizado
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(Some(serde_json::json!({
            "job_id": job_id,
            "handler": handler,
            "payload": payload,
            "status": "completed"
        })))
    }

    /// Processa resultado de job
    async fn handle_job_result(&self, result: JobResult) -> Result<(), Box<dyn std::error::Error>> {
        let job_id = result.job_id;

        // Atualiza execução
        {
            let mut running = self.running_jobs.write().unwrap();
            if let Some(execution) = running.remove(&job_id) {
                let final_execution = JobExecution {
                    job_id,
                    status: if result.success { JobStatus::Completed } else { JobStatus::Failed },
                    started_at: execution.started_at,
                    completed_at: Some(self.current_tick()),
                    traj_used: result.traj_used,
                    error_message: result.error,
                    result: result.result,
                    span_id: execution.span_id,
                };

                // Move para histórico
                let mut history = self.job_history.write().unwrap();
                history.insert(job_id, final_execution);
            }
        }

        // Atualiza estatísticas
        {
            let mut stats = self.stats.write().unwrap();
            stats.jobs_running = stats.jobs_running.saturating_sub(1);
            if result.success {
                stats.total_jobs_completed += 1;
            } else {
                stats.total_jobs_failed += 1;
            }
            stats.total_trajs_used += result.traj_used;
            
            // Recalcula média
            let total_completed = stats.total_jobs_completed + stats.total_jobs_failed;
            if total_completed > 0 {
                stats.average_job_duration_trajs = stats.total_trajs_used as f64 / total_completed as f64;
            }
        }

        // Emite span de conclusão usando uma cópia da informação necessária
        let result_copy = JobExecutionResult {
            job_id: result.job_id.clone(),
            execution_id: result.execution_id.clone(),
            status: result.status.clone(),
            start_time: result.start_time,
            end_time: result.end_time,
            traj_used: result.traj_used,
            error_message: result.error_message.clone(),
            result: None, // Não precisamos do resultado para o span
        };
        self.emit_job_completion_span(&result_copy).await?;

        Ok(())
    }

    /// Calcula score de prioridade
    fn calculate_priority_score(&self, job: &ScheduledJob) -> u64 {
        let current_tick = self.current_tick();
        let base_priority = (job.priority as u64) * 1_000_000;
        
        // Urgência temporal (quanto mais atrasado, maior prioridade)
        let urgency = if job.scheduled_tick <= current_tick {
            current_tick - job.scheduled_tick
        } else {
            0
        };

        base_priority + urgency
    }

    /// Retorna tick atual
    fn current_tick(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Retorna estatísticas
    pub fn get_stats(&self) -> SchedulerStats {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    /// Emite span de início de job
    async fn emit_job_start_span(&self, job: &ScheduledJob) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "job_start",
            "job_id": job.job_id,
            "job_type": job.job_type,
            "priority": job.priority,
            "traj_budget": job.traj_budget,
            "scheduled_tick": job.scheduled_tick
        });

        self.span_emitter.emit_span(
            "job_start",
            "scheduler",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de conclusão de job
    async fn emit_job_completion_span(&self, result: &JobResult) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "job_completion",
            "job_id": result.job_id,
            "success": result.success,
            "traj_used": result.traj_used,
            "error": result.error
        });

        self.span_emitter.emit_span(
            "job_completion",
            "scheduler",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }
}

impl Clone for Scheduler {
    fn clone(&self) -> Self {
        Self {
            id_with_keys: self.id_with_keys.clone(),
            span_emitter: Arc::clone(&self.span_emitter),
            pending_jobs: Arc::clone(&self.pending_jobs),
            running_jobs: Arc::clone(&self.running_jobs),
            job_history: Arc::clone(&self.job_history),
            job_sender: Arc::clone(&self.job_sender),
            job_receiver: Arc::clone(&self.job_receiver),
            result_sender: Arc::clone(&self.result_sender),
            result_receiver: Arc::clone(&self.result_receiver),
            max_concurrent_jobs: self.max_concurrent_jobs,
            default_traj_budget: self.default_traj_budget,
            default_timeout_ticks: self.default_timeout_ticks,
            stats: Arc::clone(&self.stats),
        }
    }
}

/// Listener de ticks para o Scheduler
pub struct SchedulerTickListener {
    scheduler: Arc<Scheduler>,
}

impl SchedulerTickListener {
    pub fn new(scheduler: Arc<Scheduler>) -> Self {
        Self { scheduler }
    }
}

impl TickListener for SchedulerTickListener {
    fn on_tick(&self, tick: u64, _rotation_count: u64) {
        // Verifica jobs prontos para execução
        let ready_jobs = {
            let mut pending = self.scheduler.pending_jobs.lock().unwrap();
            let mut ready = Vec::new();
            let mut temp_heap = BinaryHeap::new();

            while let Some(pending_job) = pending.pop() {
                if pending_job.job.scheduled_tick <= tick {
                    // Verifica dependências
                    let dependencies_met = pending_job.job.dependencies.iter().all(|dep_id| {
                        let history = self.scheduler.job_history.read().unwrap();
                        history.get(dep_id)
                            .map(|execution| matches!(execution.status, JobStatus::Completed))
                            .unwrap_or(false)
                    });

                    if dependencies_met {
                        ready.push(pending_job.job);
                    } else {
                        temp_heap.push(pending_job);
                    }
                } else {
                    temp_heap.push(pending_job);
                }
            }

            // Restaura jobs não prontos
            *pending = temp_heap;
            ready
        };

        // Verifica limitações de concorrência
        let current_running = {
            let running = self.scheduler.running_jobs.read().unwrap();
            running.len()
        };

        let available_slots = self.scheduler.max_concurrent_jobs.saturating_sub(current_running);

        // Envia jobs prontos para execução (respeitando limite de concorrência)
        for job in ready_jobs.into_iter().take(available_slots) {
            if let Some(sender) = self.scheduler.job_sender.lock().unwrap().as_ref() {
                let _ = sender.send(job);
            }
        }

        // Verifica timeouts
        self.check_job_timeouts(tick);
    }

    fn on_drift_detected(&self, _drift_micros: i64) {
        // Pode implementar lógica específica para drift
    }

    fn on_emergency_stop(&self, _reason: &str) {
        // Para todos os jobs em execução
        let mut running = self.scheduler.running_jobs.write().unwrap();
        for execution in running.values_mut() {
            execution.status = JobStatus::Failed;
            execution.error_message = Some("Emergency stop".to_string());
            execution.completed_at = Some(self.scheduler.current_tick());
        }
    }
}

impl SchedulerTickListener {
    fn check_job_timeouts(&self, current_tick: u64) {
        let mut timed_out_jobs = Vec::new();
        
        {
            let running = self.scheduler.running_jobs.read().unwrap();
            for (job_id, execution) in running.iter() {
                if let Some(started_at) = execution.started_at {
                    // Busca timeout do job original (simplificado)
                    let timeout_duration = self.scheduler.default_timeout_ticks;
                    
                    if current_tick - started_at > timeout_duration {
                        timed_out_jobs.push(*job_id);
                    }
                }
            }
        }

        // Marca jobs como timeout
        if !timed_out_jobs.is_empty() {
            let mut running = self.scheduler.running_jobs.write().unwrap();
            for job_id in timed_out_jobs {
                if let Some(execution) = running.get_mut(&job_id) {
                    execution.status = JobStatus::Timeout;
                    execution.completed_at = Some(current_tick);
                    execution.error_message = Some("Job timeout".to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motor::span::SpanEmitter;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let scheduler = Scheduler::new(id_with_keys, span_emitter);
        
        let stats = scheduler.get_stats();
        assert_eq!(stats.total_jobs_scheduled, 0);
        assert_eq!(stats.jobs_in_queue, 0);
    }

    #[tokio::test]
    async fn test_schedule_lab_job() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let scheduler = Scheduler::new(id_with_keys, span_emitter);
        
        let job_id = scheduler.schedule_lab_job(
            "transcode",
            "/input/video.mp4",
            "/output/video.webm",
            Some(5000),
            None,
        ).unwrap();
        
        assert!(!job_id.is_nil());
        
        let stats = scheduler.get_stats();
        assert_eq!(stats.total_jobs_scheduled, 1);
        assert_eq!(stats.jobs_in_queue, 1);
    }
}