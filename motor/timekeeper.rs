/// # LogLine Runtime Engine - Timekeeper
/// 
/// O relógio computável fundamental do sistema LogLine.
/// Responsável por:
/// - Emitir ticks precisos a cada 64μs (1 traj)  
/// - Manter contagem de rotações desde o boot
/// - Detectar drift temporal e falhas de clock
/// - Coordenar execução de filas temporais
/// - Base institucional para enforcement de prazos
/// 
/// Este módulo é o marcapasso de todo o ecossistema LogLine,
/// incluindo filas do Lab, curadoria da TV e enforcement constitucional.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::thread;
use tokio::time::interval;
use serde::{Serialize, Deserialize};
use crate::motor::span::SpanEmitter;
use crate::infra::id::logline_id::LogLineIDWithKeys;
use crate::time::adaptive_clock::{AdaptiveClock, TemporalListener};
use crate::time::time_model::{TimeState as AdaptiveTimeState, TemporalEvent, TemporalClockStatus};

/// Constante fundamental: 1 traj = 64 microssegundos
pub const TRAJ_DURATION_MICROS: u64 = 64;
pub const TICKS_PER_SECOND: u64 = 1_000_000 / TRAJ_DURATION_MICROS; // ~15,625 ticks/segundo

/// Estado temporal do sistema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeState {
    /// Timestamp do último tick em microssegundos UNIX
    pub last_tick: u64,
    /// Intervalo entre ticks (sempre 64μs)
    pub tick_interval: u64,
    /// Contagem total de rotações desde o boot
    pub rotation_count: u64,
    /// Se drift temporal foi detectado
    pub drift_detected: bool,
    /// Timestamp de início do motor
    pub boot_time: u64,
    /// Status do relógio
    pub clock_status: ClockStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClockStatus {
    Running,
    Drifting,
    Stopped,
    Critical,
}

/// Modo de operação do timekeeper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimekeeperMode {
    /// Modo tradicional: ticks fixos de 64μs
    FixedTick,
    /// Modo adaptativo: usa modelo de tempo da gramática
    Adaptive,
    /// Modo híbrido: 64μs + eventos adaptativos
    Hybrid,
}

/// Handle do Timekeeper para controle externo
pub struct TimekeeperHandle {
    pub time_state: Arc<Mutex<TimeState>>,
    pub is_running: Arc<AtomicBool>,
    pub emergency_stop: Arc<AtomicBool>,
}

/// Listener para eventos de tick
pub trait TickListener: Send + Sync {
    fn on_tick(&self, tick: u64, rotation_count: u64);
    fn on_drift_detected(&self, drift_micros: i64);
    fn on_emergency_stop(&self, reason: &str);
}

/// Motor de tempo computável principal
pub struct Timekeeper {
    id_with_keys: LogLineIDWithKeys,
    span_emitter: Arc<SpanEmitter>,
    time_state: Arc<Mutex<TimeState>>,
    is_running: Arc<AtomicBool>,
    emergency_stop: Arc<AtomicBool>,
    listeners: Arc<Mutex<Vec<Arc<dyn TickListener + Send + Sync>>>>,
    drift_threshold_micros: i64,
    max_missed_ticks: u64,
    
    /// Clock adaptativo opcional (quando gramática definir modelo de tempo)
    adaptive_clock: Arc<Mutex<Option<AdaptiveClock>>>,
    
    /// Modo de operação
    operation_mode: Arc<Mutex<TimekeeperMode>>,
}

impl Timekeeper {
    /// Cria nova instância do Timekeeper
    pub fn new(id_with_keys: LogLineIDWithKeys, span_emitter: Arc<SpanEmitter>) -> Self {
        let boot_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let time_state = TimeState {
            last_tick: boot_time,
            tick_interval: TRAJ_DURATION_MICROS,
            rotation_count: 0,
            drift_detected: false,
            boot_time,
            clock_status: ClockStatus::Stopped,
        };

        Self {
            id_with_keys,
            span_emitter,
            time_state: Arc::new(Mutex::new(time_state)),
            is_running: Arc::new(AtomicBool::new(false)),
            emergency_stop: Arc::new(AtomicBool::new(false)),
            listeners: Arc::new(Mutex::new(Vec::new())),
            drift_threshold_micros: 10, // 10μs de tolerância
            max_missed_ticks: 100, // máximo de ticks perdidos antes de emergência
            adaptive_clock: Arc::new(Mutex::new(None)),
            operation_mode: Arc::new(Mutex::new(TimekeeperMode::FixedTick)),
        }
    }

    /// Registra um listener para eventos de tick
    pub fn add_listener(&self, listener: Arc<dyn TickListener + Send + Sync>) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.push(listener);
    }

    /// Retorna handle para controle externo
    pub fn get_handle(&self) -> TimekeeperHandle {
        TimekeeperHandle {
            time_state: Arc::clone(&self.time_state),
            is_running: Arc::clone(&self.is_running),
            emergency_stop: Arc::clone(&self.emergency_stop),
        }
    }

    /// Inicia o motor de tempo
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_running.load(Ordering::Relaxed) {
            return Err("Timekeeper já está rodando".into());
        }

        self.is_running.store(true, Ordering::Relaxed);
        self.emergency_stop.store(false, Ordering::Relaxed);

        // Atualiza estado inicial
        {
            let mut state = self.time_state.lock().unwrap();
            state.clock_status = ClockStatus::Running;
            state.last_tick = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
        }

        // Emite span de início
        self.emit_boot_span().await?;

        // Inicia loop principal
        self.run_tick_loop().await?;

        Ok(())
    }

    /// Para o motor de tempo
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_running.store(false, Ordering::Relaxed);
        
        {
            let mut state = self.time_state.lock().unwrap();
            state.clock_status = ClockStatus::Stopped;
        }

        self.emit_stop_span().await?;
        Ok(())
    }

    /// Para o motor em emergência
    pub async fn emergency_stop(&self, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.emergency_stop.store(true, Ordering::Relaxed);
        self.is_running.store(false, Ordering::Relaxed);

        {
            let mut state = self.time_state.lock().unwrap();
            state.clock_status = ClockStatus::Critical;
        }

        // Notifica listeners
        let listeners = self.listeners.lock().unwrap();
        for listener in listeners.iter() {
            listener.on_emergency_stop(reason);
        }

        self.emit_emergency_span(reason).await?;
        Ok(())
    }

    /// Loop principal de ticks
    async fn run_tick_loop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = interval(Duration::from_micros(TRAJ_DURATION_MICROS));
        let mut last_expected_tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let mut missed_ticks = 0u64;

        while self.is_running.load(Ordering::Relaxed) && !self.emergency_stop.load(Ordering::Relaxed) {
            interval.tick().await;

            let now_micros = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;

            // Calcula drift
            let expected_tick = last_expected_tick + TRAJ_DURATION_MICROS;
            let drift = now_micros as i64 - expected_tick as i64;

            // Atualiza estado
            let (rotation_count, should_emit_drift) = {
                let mut state = self.time_state.lock().unwrap();
                state.last_tick = now_micros;
                state.rotation_count += 1;
                
                if drift.abs() > self.drift_threshold_micros {
                    state.drift_detected = true;
                    state.clock_status = ClockStatus::Drifting;
                    missed_ticks += 1;
                } else {
                    state.drift_detected = false;
                    if matches!(state.clock_status, ClockStatus::Drifting) {
                        state.clock_status = ClockStatus::Running;
                    }
                    missed_ticks = 0;
                }

                (state.rotation_count, state.drift_detected)
            };

            // Verifica emergência por drift excessivo
            if missed_ticks > self.max_missed_ticks {
                self.emergency_stop("Drift temporal crítico detectado").await?;
                break;
            }

            // Notifica listeners
            let listeners = self.listeners.lock().unwrap();
            for listener in listeners.iter() {
                listener.on_tick(now_micros, rotation_count);
                
                if should_emit_drift {
                    listener.on_drift_detected(drift);
                }
            }

            // Emite spans periódicos
            if rotation_count % 1000 == 0 { // A cada ~64ms
                self.emit_tick_span(rotation_count, drift).await?;
            }

            last_expected_tick = expected_tick;
        }

        Ok(())
    }

    /// Emite span de inicialização
    async fn emit_boot_span(&self) -> Result<(), Box<dyn std::error::Error>> {
        let boot_time = {
            let state = self.time_state.lock().unwrap();
            state.boot_time
        };

        let span_data = serde_json::json!({
            "type": "timekeeper_boot",
            "boot_time": boot_time,
            "tick_interval": TRAJ_DURATION_MICROS,
            "ticks_per_second": TICKS_PER_SECOND,
            "status": "running"
        });

        self.span_emitter.emit_span(
            "timekeeper_boot",
            "system",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de parada
    async fn emit_stop_span(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (rotation_count, uptime) = {
            let state = self.time_state.lock().unwrap();
            let uptime = state.last_tick - state.boot_time;
            (state.rotation_count, uptime)
        };

        let span_data = serde_json::json!({
            "type": "timekeeper_stop",
            "final_rotation_count": rotation_count,
            "uptime_micros": uptime,
            "uptime_seconds": uptime as f64 / 1_000_000.0,
            "status": "stopped"
        });

        self.span_emitter.emit_span(
            "timekeeper_stop",
            "system",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de emergência
    async fn emit_emergency_span(&self, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (rotation_count, last_tick) = {
            let state = self.time_state.lock().unwrap();
            (state.rotation_count, state.last_tick)
        };

        let span_data = serde_json::json!({
            "type": "timekeeper_emergency",
            "reason": reason,
            "rotation_count": rotation_count,
            "last_tick": last_tick,
            "severity": "critical",
            "actions_required": [
                "mark_all_contracts_as_draft",
                "trigger_reconciliation_protocol",
                "notify_admins"
            ]
        });

        self.span_emitter.emit_span(
            "timekeeper_emergency",
            "violation",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span periódico de tick
    async fn emit_tick_span(&self, rotation_count: u64, drift: i64) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "rotation",
            "rotation_count": rotation_count,
            "drift_micros": drift,
            "clock": "global",
            "status": if drift.abs() > self.drift_threshold_micros { "drifting" } else { "ok" }
        });

        self.span_emitter.emit_span(
            "rotation",
            "system",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Retorna estado atual do tempo
    pub fn get_time_state(&self) -> TimeState {
        let state = self.time_state.lock().unwrap();
        state.clone()
    }

    /// Retorna o tick atual em microssegundos
    pub fn current_tick(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Converte microssegundos para trajs
    pub fn micros_to_trajs(micros: u64) -> u64 {
        micros / TRAJ_DURATION_MICROS
    }

    /// Converte trajs para microssegundos
    pub fn trajs_to_micros(trajs: u64) -> u64 {
        trajs * TRAJ_DURATION_MICROS
    }
}

/// Implementação de Debug para Timekeeper
impl std::fmt::Debug for Timekeeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.time_state.lock().unwrap();
        f.debug_struct("Timekeeper")
            .field("rotation_count", &state.rotation_count)
            .field("clock_status", &state.clock_status)
            .field("is_running", &self.is_running.load(Ordering::Relaxed))
            .field("drift_detected", &state.drift_detected)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_timekeeper_basic_operations() {
        // Mock components para teste
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        
        let timekeeper = Timekeeper::new(id_with_keys, span_emitter);
        
        // Testa estado inicial
        let state = timekeeper.get_time_state();
        assert_eq!(state.rotation_count, 0);
        assert!(matches!(state.clock_status, ClockStatus::Stopped));
        
        // Testa conversões
        assert_eq!(Timekeeper::trajs_to_micros(1), 64);
        assert_eq!(Timekeeper::micros_to_trajs(128), 2);
    }

    #[tokio::test] 
    async fn test_timekeeper_start_stop() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let timekeeper = Timekeeper::new(id_with_keys, span_emitter);
        
        // Inicia timekeeper em thread separada
        let timekeeper_clone = Arc::new(timekeeper);
        let handle = timekeeper_clone.get_handle();
        
        tokio::spawn({
            let tk = Arc::clone(&timekeeper_clone);
            async move {
                let _ = tk.start().await;
            }
        });
        
        // Aguarda alguns ticks
        sleep(Duration::from_millis(1)).await;
        
        // Verifica se está rodando
        assert!(handle.is_running.load(Ordering::Relaxed));
        
        // Para o timekeeper
        let _ = timekeeper_clone.stop().await;
        
        sleep(Duration::from_millis(1)).await;
        assert!(!handle.is_running.load(Ordering::Relaxed));
    }
}