/// # LogLine Runtime Engine - Executor
///
/// Executor de contratos .lll, jobs e agentes computacionais.
/// Responsável por:
/// - Execução segura de contratos .lll com contagem de trajs
/// - Processamento de jobs do Lab e curadoria da TV
/// - Execução de agentes LLM e computacionais
/// - Geração de receipts computáveis
/// - Enforcement de limites de recursos e timeouts
/// - Integração com sistema de rollback e auditoria
///
/// O Executor é o módulo que realmente executa o código,
/// seja contratos LogLine, jobs de processamento ou agentes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use std::process::{Command, Stdio};
use tokio::time::timeout;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::motor::span::SpanEmitter;
use crate::motor::timekeeper::{Timekeeper, TRAJ_DURATION_MICROS};
use crate::motor::scheduler::{JobType, Priority};
use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
use crate::enforcement::{Agent, Role, EnforcementValidation};

/// Contexto de execução
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub execution_id: Uuid,
    pub executor_id: LogLineID,
    pub start_time: u64,
    pub traj_budget: u64,
    pub traj_used: u64,
    pub timeout: Option<u64>,
    pub priority: Priority,
    pub environment: HashMap<String, String>,
    pub working_directory: String,
    pub enforcement_active: bool,
}

/// Resultado de execução
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub execution_id: Uuid,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub traj_used: u64,
    pub duration_micros: u64,
    pub output: Option<String>,
    pub error: Option<String>,
    pub receipt: Option<ExecutionReceipt>,
    pub spans_generated: Vec<String>,
}

/// Recibo computável de execução
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub receipt_id: Uuid,
    pub execution_id: Uuid,
    pub contract_hash: Option<String>,
    pub executor_signature: String,
    pub timestamp: u64,
    pub traj_cost: u64,
    pub enforcement_status: EnforcementValidation,
    pub metadata: serde_json::Value,
}

/// Status de execução
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Queued,
    Starting,
    Running,
    Completed,
    Failed,
    Timeout,
    Cancelled,
    RolledBack,
}

/// Execução ativa
#[derive(Debug)]
pub struct ActiveExecution {
    pub context: ExecutionContext,
    pub status: ExecutionStatus,
    pub start_instant: Instant,
    pub cancel_handle: tokio::sync::oneshot::Sender<()>,
}

/// Configuração do executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    pub max_concurrent_executions: usize,
    pub default_timeout_seconds: u64,
    pub max_traj_per_execution: u64,
    pub enable_sandboxing: bool,
    pub working_dir: String,
    pub temp_dir: String,
}

/// Motor de execução principal
pub struct Executor {
    id_with_keys: LogLineIDWithKeys,
    span_emitter: Arc<SpanEmitter>,
    config: ExecutorConfig,
    
    // Execuções ativas
    active_executions: Arc<RwLock<HashMap<Uuid, ActiveExecution>>>,
    
    // Histórico de execuções (limitado)
    execution_history: Arc<RwLock<HashMap<Uuid, ExecutionResult>>>,
    
    // Contadores de recursos
    total_trajs_used: Arc<Mutex<u64>>,
    total_executions: Arc<Mutex<u64>>,
}

impl Executor {
    /// Cria nova instância do Executor
    pub fn new(
        id_with_keys: LogLineIDWithKeys,
        span_emitter: Arc<SpanEmitter>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            id_with_keys,
            span_emitter,
            config,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            execution_history: Arc::new(RwLock::new(HashMap::new())),
            total_trajs_used: Arc::new(Mutex::new(0)),
            total_executions: Arc::new(Mutex::new(0)),
        }
    }

    /// Executa contrato .lll
    pub async fn execute_contract(
        &self,
        contract_path: &str,
        traj_budget: u64,
        priority: Priority,
        environment: HashMap<String, String>,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let context = self.create_execution_context(traj_budget, priority, environment)?;
        
        // Emite span de início
        self.emit_execution_start_span(&context, "contract", contract_path).await?;

        // Verifica se arquivo existe
        if !std::path::Path::new(contract_path).exists() {
            return Err(format!("Contrato não encontrado: {}", contract_path).into());
        }

        // Lê e valida contrato
        let contract_content = std::fs::read_to_string(contract_path)?;
        let contract_hash = self.calculate_contract_hash(&contract_content);

        // Executa contrato via CLI do LogLine
        let result = self.execute_logline_contract(&context, contract_path, &contract_hash).await?;

        // Gera recibo se execução foi bem-sucedida
        let receipt = if result.success {
            Some(self.generate_receipt(&context, &result, Some(contract_hash)).await?)
        } else {
            None
        };

        let final_result = ExecutionResult {
            receipt,
            ..result
        };

        // Armazena no histórico
        self.store_execution_result(&final_result).await?;

        // Emite span de conclusão
        self.emit_execution_completion_span(&final_result).await?;

        Ok(final_result)
    }

    /// Executa job do Lab
    pub async fn execute_lab_job(
        &self,
        operation: &str,
        input_path: &str,
        output_path: &str,
        traj_budget: u64,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let context = self.create_execution_context(traj_budget, Priority::Normal, HashMap::new())?;
        
        self.emit_execution_start_span(&context, "lab_job", operation).await?;

        let result = match operation {
            "transcode" => self.execute_transcode(&context, input_path, output_path).await?,
            "analyze" => self.execute_analysis(&context, input_path, output_path).await?,
            "compress" => self.execute_compression(&context, input_path, output_path).await?,
            "extract_frames" => self.execute_frame_extraction(&context, input_path, output_path).await?,
            _ => return Err(format!("Operação de Lab desconhecida: {}", operation).into()),
        };

        self.store_execution_result(&result).await?;
        self.emit_execution_completion_span(&result).await?;

        Ok(result)
    }

    /// Executa job de curadoria da TV
    pub async fn execute_tv_job(
        &self,
        operation: &str,
        video_id: &str,
        target_slot: Option<u64>,
        traj_budget: u64,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let context = self.create_execution_context(traj_budget, Priority::High, HashMap::new())?;
        
        self.emit_execution_start_span(&context, "tv_job", operation).await?;

        let result = match operation {
            "download" => self.execute_tv_download(&context, video_id).await?,
            "edit" => self.execute_tv_edit(&context, video_id).await?,
            "upload" => self.execute_tv_upload(&context, video_id).await?,
            "schedule" => self.execute_tv_schedule(&context, video_id, target_slot).await?,
            _ => return Err(format!("Operação de TV desconhecida: {}", operation).into()),
        };

        self.store_execution_result(&result).await?;
        self.emit_execution_completion_span(&result).await?;

        Ok(result)
    }

    /// Executa agente
    pub async fn execute_agent(
        &self,
        agent_id: &str,
        command: &str,
        context_data: serde_json::Value,
        traj_budget: u64,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let context = self.create_execution_context(traj_budget, Priority::Normal, HashMap::new())?;
        
        self.emit_execution_start_span(&context, "agent", agent_id).await?;

        let result = match agent_id {
            "curador_tv" => self.execute_tv_curator_agent(&context, command, &context_data).await?,
            "lab_processor" => self.execute_lab_processor_agent(&context, command, &context_data).await?,
            "institutional" => self.execute_institutional_agent(&context, command, &context_data).await?,
            _ => return Err(format!("Agente desconhecido: {}", agent_id).into()),
        };

        self.store_execution_result(&result).await?;
        self.emit_execution_completion_span(&result).await?;

        Ok(result)
    }

    /// Cria contexto de execução
    fn create_execution_context(
        &self,
        traj_budget: u64,
        priority: Priority,
        environment: HashMap<String, String>,
    ) -> Result<ExecutionContext, Box<dyn std::error::Error>> {
        let execution_id = Uuid::new_v4();
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_micros() as u64;

        Ok(ExecutionContext {
            execution_id,
            executor_id: self.id_with_keys.id.clone(),
            start_time,
            traj_budget,
            traj_used: 0,
            timeout: Some(start_time + (self.config.default_timeout_seconds * 1_000_000)),
            priority,
            environment,
            working_directory: self.config.working_dir.clone(),
            enforcement_active: true,
        })
    }

    /// Executa contrato LogLine via CLI
    async fn execute_logline_contract(
        &self,
        context: &ExecutionContext,
        contract_path: &str,
        contract_hash: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Registra execução ativa
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        {
            let mut active = self.active_executions.write().unwrap();
            active.insert(context.execution_id, ActiveExecution {
                context: context.clone(),
                status: ExecutionStatus::Running,
                start_instant,
                cancel_handle: cancel_tx,
            });
        }

        // Executa comando LogLine
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--", "exec", "--file", contract_path])
            .current_dir(&context.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Adiciona variáveis de ambiente
        for (key, value) in &context.environment {
            cmd.env(key, value);
        }

        let result = if let Some(timeout_micros) = context.timeout {
            let timeout_duration = Duration::from_micros(timeout_micros - context.start_time);
            timeout(timeout_duration, self.run_command_with_cancel(cmd, cancel_rx)).await
        } else {
            Ok(self.run_command_with_cancel(cmd, cancel_rx).await)
        };

        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        // Remove da lista de execuções ativas
        {
            let mut active = self.active_executions.write().unwrap();
            active.remove(&context.execution_id);
        }

        // Atualiza contadores
        {
            let mut total_trajs = self.total_trajs_used.lock().unwrap();
            *total_trajs += traj_used;
            
            let mut total_exec = self.total_executions.lock().unwrap();
            *total_exec += 1;
        }

        match result {
            Ok(Ok(output)) => Ok(ExecutionResult {
                execution_id: context.execution_id,
                success: output.success,
                exit_code: output.exit_code,
                traj_used,
                duration_micros: duration.as_micros() as u64,
                output: output.stdout,
                error: output.stderr,
                receipt: None,
                spans_generated: vec![],
            }),
            Ok(Err(e)) => Ok(ExecutionResult {
                execution_id: context.execution_id,
                success: false,
                exit_code: None,
                traj_used,
                duration_micros: duration.as_micros() as u64,
                output: None,
                error: Some(e.to_string()),
                receipt: None,
                spans_generated: vec![],
            }),
            Err(_) => Ok(ExecutionResult {
                execution_id: context.execution_id,
                success: false,
                exit_code: None,
                traj_used,
                duration_micros: duration.as_micros() as u64,
                output: None,
                error: Some("Execution timeout".to_string()),
                receipt: None,
                spans_generated: vec![],
            }),
        }
    }

    /// Executa comando com possibilidade de cancelamento
    async fn run_command_with_cancel(
        &self,
        mut cmd: Command,
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        let child = cmd.spawn()?;
        
        tokio::select! {
            result = self.wait_for_command(child) => result,
            _ = cancel_rx => {
                Err("Execution cancelled".into())
            }
        }
    }

    /// Aguarda conclusão do comando
    async fn wait_for_command(&self, mut child: std::process::Child) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        let output = child.wait_with_output()?;
        
        Ok(CommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string().into(),
            stderr: if output.stderr.is_empty() { 
                None 
            } else { 
                Some(String::from_utf8_lossy(&output.stderr).to_string()) 
            },
        })
    }

    /// Executa transcodificação (Lab)
    async fn execute_transcode(
        &self,
        context: &ExecutionContext,
        input_path: &str,
        output_path: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de transcodificação
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Transcoded {} to {}", input_path, output_path)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa análise (Lab)
    async fn execute_analysis(
        &self,
        context: &ExecutionContext,
        input_path: &str,
        output_path: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de análise
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Analyzed {} -> {}", input_path, output_path)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa compressão (Lab)
    async fn execute_compression(
        &self,
        context: &ExecutionContext,
        input_path: &str,
        output_path: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de compressão
        tokio::time::sleep(Duration::from_millis(800)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Compressed {} to {}", input_path, output_path)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa extração de frames (Lab)
    async fn execute_frame_extraction(
        &self,
        context: &ExecutionContext,
        input_path: &str,
        output_path: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de extração de frames
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Extracted frames from {} to {}", input_path, output_path)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa download da TV
    async fn execute_tv_download(
        &self,
        context: &ExecutionContext,
        video_id: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de download
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Downloaded video {}", video_id)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa edição da TV
    async fn execute_tv_edit(
        &self,
        context: &ExecutionContext,
        video_id: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de edição
        tokio::time::sleep(Duration::from_millis(2000)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Edited video {}", video_id)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa upload da TV
    async fn execute_tv_upload(
        &self,
        context: &ExecutionContext,
        video_id: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de upload
        tokio::time::sleep(Duration::from_millis(1500)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Uploaded video {}", video_id)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa agendamento da TV
    async fn execute_tv_schedule(
        &self,
        context: &ExecutionContext,
        video_id: &str,
        target_slot: Option<u64>,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de agendamento
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Scheduled video {} for slot {:?}", video_id, target_slot)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa agente curador da TV
    async fn execute_tv_curator_agent(
        &self,
        context: &ExecutionContext,
        command: &str,
        context_data: &serde_json::Value,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de execução do agente curador
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("TV Curator executed command: {}", command)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa agente processador do Lab
    async fn execute_lab_processor_agent(
        &self,
        context: &ExecutionContext,
        command: &str,
        context_data: &serde_json::Value,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de execução do agente do Lab
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Lab Processor executed command: {}", command)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Executa agente institucional
    async fn execute_institutional_agent(
        &self,
        context: &ExecutionContext,
        command: &str,
        context_data: &serde_json::Value,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        let start_instant = Instant::now();
        
        // Simulação de execução do agente institucional
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let duration = start_instant.elapsed();
        let traj_used = duration.as_micros() as u64 / TRAJ_DURATION_MICROS;

        Ok(ExecutionResult {
            execution_id: context.execution_id,
            success: true,
            exit_code: Some(0),
            traj_used,
            duration_micros: duration.as_micros() as u64,
            output: Some(format!("Institutional Agent executed command: {}", command)),
            error: None,
            receipt: None,
            spans_generated: vec![],
        })
    }

    /// Gera recibo de execução
    async fn generate_receipt(
        &self,
        context: &ExecutionContext,
        result: &ExecutionResult,
        contract_hash: Option<String>,
    ) -> Result<ExecutionReceipt, Box<dyn std::error::Error>> {
        let receipt_id = Uuid::new_v4();
        
        // Simula geração de assinatura
        let signature_data = format!("{}:{}:{}", receipt_id, context.execution_id, result.traj_used);
        let executor_signature = format!("sig_{}", self.calculate_hash(&signature_data));

        Ok(ExecutionReceipt {
            receipt_id,
            execution_id: context.execution_id,
            contract_hash,
            executor_signature,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_micros() as u64,
            traj_cost: result.traj_used,
            enforcement_status: EnforcementValidation {
                validity_status: if result.success { "valid" } else { "invalid" }.to_string(),
                agent: "system".to_string(),
                inherited_rules: Vec::new(),
                execution_context: context.clone(),
            },
            metadata: serde_json::json!({
                "priority": context.priority,
                "duration_micros": result.duration_micros,
                "success": result.success
            }),
        })
    }

    /// Calcula hash de contrato
    fn calculate_contract_hash(&self, content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("contract_{:x}", hasher.finish())
    }

    /// Calcula hash genérico
    fn calculate_hash(&self, data: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Armazena resultado de execução
    async fn store_execution_result(&self, result: &ExecutionResult) -> Result<(), Box<dyn std::error::Error>> {
        let mut history = self.execution_history.write().unwrap();
        
        // Limita histórico a 1000 entradas
        if history.len() >= 1000 {
            // Remove a entrada mais antiga (simplificado)
            if let Some(oldest_key) = history.keys().next().cloned() {
                history.remove(&oldest_key);
            }
        }
        
        history.insert(result.execution_id, result.clone());
        Ok(())
    }

    /// Emite span de início de execução
    async fn emit_execution_start_span(
        &self,
        context: &ExecutionContext,
        execution_type: &str,
        target: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "execution_start",
            "execution_id": context.execution_id,
            "execution_type": execution_type,
            "target": target,
            "traj_budget": context.traj_budget,
            "priority": context.priority
        });

        self.span_emitter.emit_span(
            "execution_start",
            "executor",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de conclusão de execução
    async fn emit_execution_completion_span(&self, result: &ExecutionResult) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "execution_completion",
            "execution_id": result.execution_id,
            "success": result.success,
            "traj_used": result.traj_used,
            "duration_micros": result.duration_micros,
            "has_receipt": result.receipt.is_some()
        });

        self.span_emitter.emit_span(
            "execution_completion",
            "executor",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Cancela execução
    pub fn cancel_execution(&self, execution_id: &Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let mut active = self.active_executions.write().unwrap();
        if let Some(execution) = active.remove(execution_id) {
            let _ = execution.cancel_handle.send(());
        }
        Ok(())
    }

    /// Retorna estatísticas
    pub fn get_stats(&self) -> (u64, u64, usize) {
        let total_trajs = *self.total_trajs_used.lock().unwrap();
        let total_executions = *self.total_executions.lock().unwrap();
        let active_count = self.active_executions.read().unwrap().len();
        
        (total_trajs, total_executions, active_count)
    }
}

/// Saída de comando
#[derive(Debug)]
struct CommandOutput {
    success: bool,
    exit_code: Option<i32>,
    stdout: Option<String>,
    stderr: Option<String>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 4,
            default_timeout_seconds: 300, // 5 minutos
            max_traj_per_execution: 1_000_000, // ~64 segundos
            enable_sandboxing: false,
            working_dir: std::env::current_dir()
                .unwrap_or_else(|_| "/tmp".into())
                .to_string_lossy().to_string(),
            temp_dir: std::env::temp_dir().to_string_lossy().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motor::span::SpanEmitter;

    #[tokio::test]
    async fn test_executor_creation() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let config = ExecutorConfig::default();
        let executor = Executor::new(id_with_keys, span_emitter, config);
        
        let (total_trajs, total_executions, active_count) = executor.get_stats();
        assert_eq!(total_trajs, 0);
        assert_eq!(total_executions, 0);
        assert_eq!(active_count, 0);
    }

    #[tokio::test]
    async fn test_lab_job_execution() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let config = ExecutorConfig::default();
        let executor = Executor::new(id_with_keys, span_emitter, config);
        
        let result = executor.execute_lab_job(
            "transcode",
            "/input/test.mp4",
            "/output/test.webm",
            5000,
        ).await.unwrap();
        
        assert!(result.success);
        assert!(result.traj_used > 0);
        assert!(result.output.is_some());
    }
}