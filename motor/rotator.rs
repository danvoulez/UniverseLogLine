/// # LogLine Runtime Engine - Rotator  
///
/// Mecanismo binário de rotação para coordenação de motores LogLine.
/// Responsável por:
/// - Ativação de regras com on_tick, on_schedule, on_prazo
/// - Modo de operação strict ou adaptive
/// - Coordenação binária entre motores distribuídos
/// - Sincronização de estado entre nós federados
/// - Detecção de motores travados ou offline
///
/// O Rotator garante que múltiplos motores LogLine operem
/// de forma coordenada, especialmente para curadoria da TV,
/// fila do Lab e enforcement distribuído.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{interval, Instant};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::motor::timekeeper::{Timekeeper, TickListener, TRAJ_DURATION_MICROS};
use crate::motor::span::SpanEmitter;
use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
// Removed enforcement dependency - motor only handles technical coordination

/// Modo de operação do rotator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationMode {
    /// Modo strict: falha se algum motor não responder
    Strict,
    /// Modo adaptive: continua mesmo com motores offline
    Adaptive,
}

/// Status de um motor na rede
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MotorStatus {
    Active,
    Lagging,
    Offline,
    Failed,
}

/// Informações de um motor na federação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorInfo {
    pub motor_id: LogLineID,
    pub last_seen: u64,
    pub rotation_count: u64,
    pub status: MotorStatus,
    pub capabilities: Vec<String>,
    pub location: String, // "mac_mini", "railway", "android_box", etc.
}

/// Regra de ativação temporal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationRule {
    pub rule_id: String,
    pub name: String,
    pub triggers: Vec<ActivationTrigger>,
    pub target_motors: Vec<LogLineID>, // Vazio = todos os motores
    pub payload: serde_json::Value,
    pub priority: u8, // 0 = baixa, 255 = crítica
}

/// Tipos de gatilho para ativação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivationTrigger {
    /// Ativa a cada N ticks
    OnTick { interval: u64 },
    /// Ativa em timestamp específico
    OnSchedule { timestamp: u64 },
    /// Ativa quando prazo expirar
    OnPrazo { deadline: u64 },
    /// Ativa quando motor entrar em status específico
    OnMotorStatus { motor_id: LogLineID, status: MotorStatus },
    /// Ativa quando drift for detectado
    OnDrift { threshold_micros: i64 },
}

/// Evento de rotação para sincronização
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEvent {
    pub event_id: Uuid,
    pub motor_id: LogLineID,
    pub rotation_count: u64,
    pub timestamp: u64,
    pub event_type: RotationEventType,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationEventType {
    Heartbeat,
    RuleActivation,
    MotorJoin,
    MotorLeave,
    Emergency,
    Sync,
}

/// Motor principal de rotação
pub struct Rotator {
    motor_id: LogLineID,
    id_with_keys: LogLineIDWithKeys,
    span_emitter: Arc<SpanEmitter>,
    mode: RotationMode,
    
    // Estado dos motores
    motors: Arc<RwLock<HashMap<LogLineID, MotorInfo>>>,
    
    // Regras de ativação
    activation_rules: Arc<RwLock<Vec<ActivationRule>>>,
    
    // Fila de eventos
    event_queue: Arc<Mutex<VecDeque<RotationEvent>>>,
    
    // Configurações
    heartbeat_interval: Duration,
    motor_timeout: Duration,
    max_drift_tolerance: i64,
}

impl Rotator {
    /// Cria nova instância do Rotator
    pub fn new(
        id_with_keys: LogLineIDWithKeys,
        span_emitter: Arc<SpanEmitter>,
        mode: RotationMode,
    ) -> Self {
        let motor_id = id_with_keys.id.clone();

        Self {
            motor_id: motor_id.clone(),
            id_with_keys,
            span_emitter,
            mode,
            motors: Arc::new(RwLock::new(HashMap::new())),
            activation_rules: Arc::new(RwLock::new(Vec::new())),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            heartbeat_interval: Duration::from_millis(100), // ~1560 ticks
            motor_timeout: Duration::from_secs(5),
            max_drift_tolerance: 1000, // 1ms
        }
    }

    /// Registra este motor na federação
    pub async fn join_federation(&self, capabilities: Vec<String>, location: &str) -> Result<(), Box<dyn std::error::Error>> {
        let motor_info = MotorInfo {
            motor_id: self.motor_id.clone(),
            last_seen: self.current_tick(),
            rotation_count: 0,
            status: MotorStatus::Active,
            capabilities,
            location: location.to_string(),
        };

        // Adiciona a si mesmo
        {
            let mut motors = self.motors.write().unwrap();
            motors.insert(self.motor_id.clone(), motor_info.clone());
        }

        // Emite evento de join
        self.emit_rotation_event(RotationEventType::MotorJoin, Some(serde_json::to_value(&motor_info)?)).await?;

        // Emite span
        self.emit_join_span(&motor_info).await?;

        Ok(())
    }

    /// Remove motor da federação
    pub async fn leave_federation(&self) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut motors = self.motors.write().unwrap();
            motors.remove(&self.motor_id);
        }

        self.emit_rotation_event(RotationEventType::MotorLeave, None).await?;
        self.emit_leave_span().await?;

        Ok(())
    }

    /// Adiciona regra de ativação
    pub fn add_activation_rule(&self, rule: ActivationRule) {
        let mut rules = self.activation_rules.write().unwrap();
        rules.push(rule);
    }

    /// Remove regra de ativação
    pub fn remove_activation_rule(&self, rule_id: &str) {
        let mut rules = self.activation_rules.write().unwrap();
        rules.retain(|r| r.rule_id != rule_id);
    }

    /// Inicia o rotator
    pub async fn start(&self, timekeeper: Arc<Timekeeper>) -> Result<(), Box<dyn std::error::Error>> {
        // Registra como listener do timekeeper
        let rotator_listener = RotatorTickListener::new(Arc::new(self.clone()));
        timekeeper.add_listener(Arc::new(rotator_listener));

        // Inicia heartbeat loop
        self.start_heartbeat_loop().await?;

        // Inicia processamento de eventos
        self.start_event_processor().await?;

        Ok(())
    }

    /// Loop de heartbeat
    async fn start_heartbeat_loop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rotator = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = interval(rotator.heartbeat_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = rotator.send_heartbeat().await {
                    eprintln!("Erro no heartbeat: {:?}", e);
                }
                
                if let Err(e) = rotator.check_motor_timeouts().await {
                    eprintln!("Erro verificando timeouts: {:?}", e);
                }
            }
        });

        Ok(())
    }

    /// Processador de eventos
    async fn start_event_processor(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rotator = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(10));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = rotator.process_events().await {
                    eprintln!("Erro processando eventos: {:?}", e);
                }
            }
        });

        Ok(())
    }

    /// Envia heartbeat para outros motores
    async fn send_heartbeat(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rotation_count = {
            let motors = self.motors.read().unwrap();
            motors.get(&self.motor_id)
                .map(|m| m.rotation_count)
                .unwrap_or(0)
        };

        self.emit_rotation_event(
            RotationEventType::Heartbeat,
            Some(serde_json::json!({
                "rotation_count": rotation_count,
                "timestamp": self.current_tick()
            }))
        ).await?;

        Ok(())
    }

    /// Verifica timeouts de motores
    async fn check_motor_timeouts(&self) -> Result<(), Box<dyn std::error::Error>> {
        let current_time = self.current_tick();
        let timeout_micros = self.motor_timeout.as_micros() as u64;
        
        let mut motors_to_timeout = Vec::new();
        
        {
            let motors = self.motors.read().unwrap();
            for (motor_id, motor_info) in motors.iter() {
                if motor_id != &self.motor_id && 
                   current_time - motor_info.last_seen > timeout_micros &&
                   motor_info.status != MotorStatus::Offline {
                    motors_to_timeout.push(motor_id.clone());
                }
            }
        }

        // Marca motores como offline
        for motor_id in motors_to_timeout {
            self.mark_motor_offline(&motor_id).await?;
        }

        Ok(())
    }

    /// Marca motor como offline
    async fn mark_motor_offline(&self, motor_id: &LogLineID) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut motors = self.motors.write().unwrap();
            if let Some(motor_info) = motors.get_mut(motor_id) {
                motor_info.status = MotorStatus::Offline;
            }
        }

        // Verifica se modo strict
        if matches!(self.mode, RotationMode::Strict) {
            self.emit_rotation_event(
                RotationEventType::Emergency,
                Some(serde_json::json!({
                    "reason": "motor_offline_strict_mode",
                    "offline_motor": motor_id.to_string()
                }))
            ).await?;
        }

        self.emit_motor_offline_span(motor_id).await?;

        Ok(())
    }

    /// Processa fila de eventos
    async fn process_events(&self) -> Result<(), Box<dyn std::error::Error>> {
        let events = {
            let mut queue = self.event_queue.lock().unwrap();
            let mut events = Vec::new();
            while let Some(event) = queue.pop_front() {
                events.push(event);
            }
            events
        };

        for event in events {
            self.handle_rotation_event(&event).await?;
        }

        Ok(())
    }

    /// Manipula evento de rotação
    async fn handle_rotation_event(&self, event: &RotationEvent) -> Result<(), Box<dyn std::error::Error>> {
        match &event.event_type {
            RotationEventType::Heartbeat => {
                self.update_motor_status(&event.motor_id, event.rotation_count, event.timestamp).await?;
            },
            RotationEventType::RuleActivation => {
                self.execute_rule_activation(event).await?;
            },
            RotationEventType::MotorJoin => {
                if let Some(payload) = &event.payload {
                    if let Ok(motor_info) = serde_json::from_value::<MotorInfo>(payload.clone()) {
                        let mut motors = self.motors.write().unwrap();
                        motors.insert(event.motor_id.clone(), motor_info);
                    }
                }
            },
            RotationEventType::Emergency => {
                self.handle_emergency_event(event).await?;
            },
            _ => {}
        }

        Ok(())
    }

    /// Atualiza status de motor
    async fn update_motor_status(&self, motor_id: &LogLineID, rotation_count: u64, timestamp: u64) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut motors = self.motors.write().unwrap();
            if let Some(motor_info) = motors.get_mut(motor_id) {
                motor_info.last_seen = timestamp;
                motor_info.rotation_count = rotation_count;
                motor_info.status = MotorStatus::Active;
            }
        }

        Ok(())
    }

    /// Executa ativação de regra
    async fn execute_rule_activation(&self, event: &RotationEvent) -> Result<(), Box<dyn std::error::Error>> {
        // Implementar lógica específica de ativação
        self.emit_rule_activation_span(&event.event_id.to_string()).await?;
        Ok(())
    }

    /// Manipula evento de emergência
    async fn handle_emergency_event(&self, event: &RotationEvent) -> Result<(), Box<dyn std::error::Error>> {
        self.emit_emergency_span(&format!("Emergência: {:?}", event.payload)).await?;
        Ok(())
    }

    /// Emite evento de rotação
    async fn emit_rotation_event(&self, event_type: RotationEventType, payload: Option<serde_json::Value>) -> Result<(), Box<dyn std::error::Error>> {
        let event = RotationEvent {
            event_id: Uuid::new_v4(),
            motor_id: self.motor_id.clone(),
            rotation_count: self.get_current_rotation_count(),
            timestamp: self.current_tick(),
            event_type,
            payload,
        };

        {
            let mut queue = self.event_queue.lock().unwrap();
            queue.push_back(event);
        }

        Ok(())
    }

    /// Retorna contagem atual de rotações
    fn get_current_rotation_count(&self) -> u64 {
        let motors = self.motors.read().unwrap();
        motors.get(&self.motor_id)
            .map(|m| m.rotation_count)
            .unwrap_or(0)
    }

    /// Retorna tick atual
    fn current_tick(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Emite span de join
    async fn emit_join_span(&self, motor_info: &MotorInfo) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "motor_join",
            "motor_id": motor_info.motor_id.to_string(),
            "location": motor_info.location,
            "capabilities": motor_info.capabilities,
            "mode": self.mode
        });

        self.span_emitter.emit_span(
            "motor_join",
            "federation",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de leave
    async fn emit_leave_span(&self) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "motor_leave",
            "motor_id": self.motor_id.to_string()
        });

        self.span_emitter.emit_span(
            "motor_leave",
            "federation",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de motor offline
    async fn emit_motor_offline_span(&self, motor_id: &LogLineID) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "motor_offline",
            "offline_motor": motor_id.to_string(),
            "mode": self.mode,
            "severity": if matches!(self.mode, RotationMode::Strict) { "critical" } else { "warning" }
        });

        self.span_emitter.emit_span(
            "motor_offline",
            "federation",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de ativação de regra
    async fn emit_rule_activation_span(&self, rule_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "rule_activation",
            "rule_id": rule_id
        });

        self.span_emitter.emit_span(
            "rule_activation",
            "enforcement",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de emergência  
    async fn emit_emergency_span(&self, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "rotator_emergency",
            "reason": reason,
            "severity": "critical"
        });

        self.span_emitter.emit_span(
            "rotator_emergency",
            "violation",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }
}

impl Clone for Rotator {
    fn clone(&self) -> Self {
        Self {
            motor_id: self.motor_id.clone(),
            id_with_keys: self.id_with_keys.clone(),
            span_emitter: Arc::clone(&self.span_emitter),
            mode: self.mode.clone(),
            motors: Arc::clone(&self.motors),
            activation_rules: Arc::clone(&self.activation_rules),
            event_queue: Arc::clone(&self.event_queue),
            heartbeat_interval: self.heartbeat_interval,
            motor_timeout: self.motor_timeout,
            max_drift_tolerance: self.max_drift_tolerance,
        }
    }
}

/// Listener de ticks para o Rotator
pub struct RotatorTickListener {
    rotator: Arc<Rotator>,
}

impl RotatorTickListener {
    pub fn new(rotator: Arc<Rotator>) -> Self {
        Self { rotator }
    }
}

impl TickListener for RotatorTickListener {
    fn on_tick(&self, tick: u64, rotation_count: u64) {
        // Atualiza contagem de rotações
        {
            let mut motors = self.rotator.motors.write().unwrap();
            if let Some(motor_info) = motors.get_mut(&self.rotator.motor_id) {
                motor_info.rotation_count = rotation_count;
                motor_info.last_seen = tick;
            }
        }

        // Verifica regras de ativação
        let rules = self.rotator.activation_rules.read().unwrap();
        for rule in rules.iter() {
            for trigger in &rule.triggers {
                match trigger {
                    ActivationTrigger::OnTick { interval } => {
                        if rotation_count % interval == 0 {
                            let _ = tokio::spawn({
                                let rotator = Arc::clone(&self.rotator);
                                let rule_id = rule.rule_id.clone();
                                async move {
                                    let _ = rotator.emit_rotation_event(
                                        RotationEventType::RuleActivation,
                                        Some(serde_json::json!({ "rule_id": rule_id }))
                                    ).await;
                                }
                            });
                        }
                    },
                    ActivationTrigger::OnSchedule { timestamp } => {
                        if tick >= *timestamp {
                            let _ = tokio::spawn({
                                let rotator = Arc::clone(&self.rotator);
                                let rule_id = rule.rule_id.clone();
                                async move {
                                    let _ = rotator.emit_rotation_event(
                                        RotationEventType::RuleActivation,
                                        Some(serde_json::json!({ "rule_id": rule_id }))
                                    ).await;
                                }
                            });
                        }
                    },
                    ActivationTrigger::OnPrazo { deadline } => {
                        if tick >= *deadline {
                            let _ = tokio::spawn({
                                let rotator = Arc::clone(&self.rotator);
                                let rule_id = rule.rule_id.clone();
                                async move {
                                    let _ = rotator.emit_rotation_event(
                                        RotationEventType::RuleActivation,
                                        Some(serde_json::json!({ "rule_id": rule_id, "trigger": "prazo_expired" }))
                                    ).await;
                                }
                            });
                        }
                    },
                    _ => {} // Outros triggers são processados em outros eventos
                }
            }
        }
    }

    fn on_drift_detected(&self, drift_micros: i64) {
        if drift_micros.abs() > self.rotator.max_drift_tolerance {
            let _ = tokio::spawn({
                let rotator = Arc::clone(&self.rotator);
                async move {
                    let _ = rotator.emit_rotation_event(
                        RotationEventType::Emergency,
                        Some(serde_json::json!({
                            "reason": "excessive_drift",
                            "drift_micros": drift_micros
                        }))
                    ).await;
                }
            });
        }
    }

    fn on_emergency_stop(&self, reason: &str) {
        let _ = tokio::spawn({
            let rotator = Arc::clone(&self.rotator);
            let reason = reason.to_string();
            async move {
                let _ = rotator.emit_rotation_event(
                    RotationEventType::Emergency,
                    Some(serde_json::json!({
                        "reason": "timekeeper_emergency",
                        "details": reason
                    }))
                ).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motor::span::SpanEmitter;

    #[tokio::test]
    async fn test_rotator_creation() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        
        let rotator = Rotator::new(id_with_keys, span_emitter, RotationMode::Adaptive);
        
        assert!(matches!(rotator.mode, RotationMode::Adaptive));
        assert_eq!(rotator.motors.read().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_rotator_federation_join() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let rotator = Rotator::new(id_with_keys, span_emitter, RotationMode::Strict);
        
        let result = rotator.join_federation(vec!["test_capability".to_string()], "test_location").await;
        assert!(result.is_ok());
        
        let motors = rotator.motors.read().unwrap();
        assert_eq!(motors.len(), 1);
        assert!(motors.contains_key(&rotator.motor_id));
    }
}