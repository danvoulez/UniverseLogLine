/// # LogLine Adaptive Clock
///
/// Clock inteligente que se adapta ao modelo de tempo
/// declarado na gramática ativa.
///
/// Substitui o timekeeper fixo de 64μs por um clock
/// que compreende diferentes unidades temporais:
/// - Dias úteis com calendário
/// - Slots de transmissão
/// - Ciclos de experimento
/// - Horários comerciais
///
/// O clock pausa, acelera ou desacelera baseado
/// no modelo temporal do projeto ativo.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{interval, Interval};
use serde::{Serialize, Deserialize};

use crate::time::time_model::{AdaptiveTimeSystem, TimeState, TemporalEvent, TemporalClockStatus};
use crate::enforcement::contextual_enforcer::TimeModel;
use crate::motor::span::SpanEmitter;
use crate::infra::id::logline_id::LogLineIDWithKeys;

/// Clock adaptativo que usa modelo de tempo da gramática
pub struct AdaptiveClock {
    id_with_keys: LogLineIDWithKeys,
    span_emitter: Arc<SpanEmitter>,
    
    /// Sistema de tempo adaptativo
    time_system: Arc<Mutex<AdaptiveTimeSystem>>,
    
    /// Configuração do clock
    config: AdaptiveClockConfig,
    
    /// Listeners para eventos temporais
    temporal_listeners: Arc<Mutex<Vec<Arc<dyn TemporalListener + Send + Sync>>>>,
    
    /// Eventos agendados
    scheduled_events: Arc<Mutex<Vec<TemporalEvent>>>,
    
    /// Estado de execução
    is_running: Arc<Mutex<bool>>,
}

/// Configuração do clock adaptativo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveClockConfig {
    /// Intervalo base de tick (em milissegundos)
    pub base_tick_interval_ms: u64,
    
    /// Fator de aceleração quando modelo permite
    pub acceleration_factor: f64,
    
    /// Pausar em horários não comerciais
    pub pause_on_non_business_hours: bool,
    
    /// Emit spans de eventos temporais
    pub emit_temporal_spans: bool,
}

/// Listener para eventos temporais
#[async_trait::async_trait]
pub trait TemporalListener {
    /// Chamado a cada tick do clock adaptativo
    async fn on_temporal_tick(&self, time_state: &TimeState) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Chamado quando evento temporal é disparado
    async fn on_temporal_event(&self, event: &TemporalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Chamado quando clock muda de status
    async fn on_clock_status_change(&self, old_status: &TemporalClockStatus, new_status: &TemporalClockStatus) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Resultado de tick temporal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalTickResult {
    pub time_state: TimeState,
    pub events_triggered: Vec<TemporalEvent>,
    pub status_changed: bool,
    pub calculations_updated: Vec<String>,
}

impl AdaptiveClock {
    /// Cria novo clock adaptativo
    pub fn new(
        id_with_keys: LogLineIDWithKeys,
        span_emitter: Arc<SpanEmitter>,
        config: AdaptiveClockConfig,
    ) -> Self {
        Self {
            id_with_keys,
            span_emitter,
            time_system: Arc::new(Mutex::new(AdaptiveTimeSystem::new())),
            config,
            temporal_listeners: Arc::new(Mutex::new(Vec::new())),
            scheduled_events: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Carrega modelo de tempo da gramática
    pub fn load_time_model(&self, time_model: TimeModel) -> Result<(), Box<dyn std::error::Error>> {
        let mut time_system = self.time_system.lock().unwrap();
        time_system.load_time_model(time_model)?;
        
        println!("🕐 Clock adaptativo configurado com modelo: {}", 
            time_system.get_active_model().unwrap().name
        );
        
        Ok(())
    }

    /// Inicia o clock adaptativo
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut running = self.is_running.lock().unwrap();
            if *running {
                return Ok(()); // Já está rodando
            }
            *running = true;
        }

        println!("🚀 Iniciando clock adaptativo...");
        
        // Emite span de início
        self.emit_clock_start_span().await?;
        
        // Inicia loop principal
        let clock = Arc::new(self.clone());
        tokio::spawn(async move {
            if let Err(e) = clock.clock_loop().await {
                eprintln!("❌ Erro no loop do clock adaptativo: {}", e);
            }
        });

        println!("✅ Clock adaptativo iniciado");
        Ok(())
    }

    /// Para o clock
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut running = self.is_running.lock().unwrap();
            *running = false;
        }

        self.emit_clock_stop_span().await?;
        println!("🛑 Clock adaptativo parado");
        Ok(())
    }

    /// Adiciona listener temporal
    pub fn add_temporal_listener(&self, listener: Arc<dyn TemporalListener + Send + Sync>) {
        let mut listeners = self.temporal_listeners.lock().unwrap();
        listeners.push(listener);
    }

    /// Agenda evento temporal
    pub fn schedule_event(&self, event: TemporalEvent) -> Result<(), Box<dyn std::error::Error>> {
        let mut events = self.scheduled_events.lock().unwrap();
        events.push(event.clone());
        
        println!("📅 Evento agendado: {} para {}", event.event_id, event.scheduled_for);
        Ok(())
    }

    /// Métodos específicos para ProjectClockManager
    
    /// Calcula tempo até próximo tick baseado no modelo ativo
    pub async fn time_until_next_tick(&self) -> Duration {
        let time_system = self.time_system.lock().unwrap();
        
        if let Some(model) = time_system.get_active_model() {
            match model.unit {
                crate::enforcement::contextual_enforcer::TimeUnit::BusinessDays => {
                    // Para dias úteis, tick a cada hora durante horário comercial
                    Duration::from_secs(3600) // 1 hora
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Hours => {
                    Duration::from_secs(3600) // 1 hora
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Minutes => {
                    Duration::from_secs(60) // 1 minuto
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Cycles => {
                    // Para ciclos experimentais, tick a cada 5 minutos
                    Duration::from_secs(300) // 5 minutos
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Slots => {
                    // Para slots de TV, tick a cada minuto
                    Duration::from_secs(60) // 1 minuto
                },
            }
        } else {
            // Sem modelo ativo, usa intervalo base
            Duration::from_millis(self.config.base_tick_interval_ms)
        }
    }

    /// Retorna tempo local atual como string formatada
    pub async fn now_local(&self) -> String {
        let time_system = self.time_system.lock().unwrap();
        let time_state = time_system.get_time_state();
        
        if let Some(model) = time_system.get_active_model() {
            match model.unit {
                crate::enforcement::contextual_enforcer::TimeUnit::BusinessDays => {
                    format!("Dia útil #{} ({})", 
                        time_state.current_unit_value as u32,
                        chrono::Utc::now().format("%Y-%m-%d")
                    )
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Hours => {
                    format!("Hora {} ({})",
                        time_state.current_unit_value as u32,
                        chrono::Utc::now().format("%H:%M")
                    )
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Cycles => {
                    format!("Ciclo #{} ({}min)",
                        time_state.current_unit_value as u32,
                        (time_state.current_unit_value * 5.0) as u32
                    )
                },
                crate::enforcement::contextual_enforcer::TimeUnit::Slots => {
                    format!("Slot {}/48 ({})",
                        time_state.current_unit_value as u32,
                        chrono::Utc::now().format("%H:%M")
                    )
                },
                _ => {
                    format!("Unidade {} = {:.2}",
                        format!("{:?}", model.unit),
                        time_state.current_unit_value
                    )
                }
            }
        } else {
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
        }
    }

    /// Emite evento de tick para listeners externos
    pub async fn emit_tick_event(&self, project_id: &str, tick_count: u64, local_time: &str) {
        let span_data = serde_json::json!({
            "type": "project_tick",
            "project_id": project_id,
            "tick_count": tick_count,
            "local_time": local_time,
            "timestamp": chrono::Utc::now()
        });

        if let Err(e) = self.span_emitter.emit_span(
            "project_clock_tick",
            "temporal_system",
            &self.id_with_keys,
            Some(span_data),
        ).await {
            eprintln!("⚠️ Erro ao emitir span de tick: {}", e);
        }
    }

    /// Retorna estado atual do tempo
    pub fn get_current_time_state(&self) -> TimeState {
        let time_system = self.time_system.lock().unwrap();
        time_system.get_time_state().clone()
    }

    /// Loop principal do clock
    async fn clock_loop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = interval(Duration::from_millis(self.config.base_tick_interval_ms));
        let mut last_status = TemporalClockStatus::Running;

        while *self.is_running.lock().unwrap() {
            interval.tick().await;
            
            // Executa tick temporal
            let tick_result = self.execute_temporal_tick().await?;
            
            // Notifica listeners sobre tick
            self.notify_temporal_tick(&tick_result.time_state).await?;
            
            // Processa eventos disparados
            for event in &tick_result.events_triggered {
                self.notify_temporal_event(event).await?;
            }
            
            // Notifica mudança de status
            if tick_result.status_changed {
                self.notify_status_change(&last_status, &tick_result.time_state.clock_status).await?;
                last_status = tick_result.time_state.clock_status.clone();
            }
            
            // Pausa se necessário (horário não comercial)
            if matches!(tick_result.time_state.clock_status, TemporalClockStatus::Paused | TemporalClockStatus::Holiday) {
                if self.config.pause_on_non_business_hours {
                    // Aumenta intervalo quando pausado
                    tokio::time::sleep(Duration::from_millis(self.config.base_tick_interval_ms * 10)).await;
                }
            }

            // Emite span periódico se configurado
            if self.config.emit_temporal_spans && tick_result.events_triggered.len() > 0 {
                self.emit_temporal_tick_span(&tick_result).await?;
            }
        }

        Ok(())
    }

    /// Executa um tick temporal
    async fn execute_temporal_tick(&self) -> Result<TemporalTickResult, Box<dyn std::error::Error>> {
        let mut time_system = self.time_system.lock().unwrap();
        let old_status = time_system.get_time_state().clone();
        
        // Atualiza estado temporal
        time_system.tick()?;
        let new_state = time_system.get_time_state().clone();
        
        // Verifica eventos a serem disparados
        let events = self.scheduled_events.lock().unwrap();
        let triggered_events = time_system.check_temporal_events(&events);
        
        // Remove eventos disparados que não são recorrentes
        drop(events);
        if !triggered_events.is_empty() {
            let mut events = self.scheduled_events.lock().unwrap();
            events.retain(|e| !triggered_events.iter().any(|te| te.event_id == e.event_id && e.recurring.is_none()));
        }

        let status_changed = !matches!((&old_status.clock_status, &new_state.clock_status), 
            (TemporalClockStatus::Running, TemporalClockStatus::Running) |
            (TemporalClockStatus::Paused, TemporalClockStatus::Paused) |
            (TemporalClockStatus::Holiday, TemporalClockStatus::Holiday)
        );

        Ok(TemporalTickResult {
            time_state: new_state,
            events_triggered: triggered_events,
            status_changed,
            calculations_updated: vec![], // TODO: track calculations
        })
    }

    /// Notifica listeners sobre tick
    async fn notify_temporal_tick(&self, time_state: &TimeState) -> Result<(), Box<dyn std::error::Error>> {
        let listeners = self.temporal_listeners.lock().unwrap();
        for listener in listeners.iter() {
            if let Err(e) = listener.on_temporal_tick(time_state).await {
                eprintln!("⚠️ Erro em listener temporal: {}", e);
            }
        }
        Ok(())
    }

    /// Notifica listeners sobre evento
    async fn notify_temporal_event(&self, event: &TemporalEvent) -> Result<(), Box<dyn std::error::Error>> {
        let listeners = self.temporal_listeners.lock().unwrap();
        for listener in listeners.iter() {
            if let Err(e) = listener.on_temporal_event(event).await {
                eprintln!("⚠️ Erro em listener de evento temporal: {}", e);
            }
        }
        Ok(())
    }

    /// Notifica mudança de status
    async fn notify_status_change(&self, old_status: &TemporalClockStatus, new_status: &TemporalClockStatus) -> Result<(), Box<dyn std::error::Error>> {
        let listeners = self.temporal_listeners.lock().unwrap();
        for listener in listeners.iter() {
            if let Err(e) = listener.on_clock_status_change(old_status, new_status).await {
                eprintln!("⚠️ Erro em listener de mudança de status: {}", e);
            }
        }
        Ok(())
    }

    /// Emite span de início do clock
    async fn emit_clock_start_span(&self) -> Result<(), Box<dyn std::error::Error>> {
        let time_system = self.time_system.lock().unwrap();
        let model_name = time_system.get_active_model()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "no_model".to_string());

        let span_data = serde_json::json!({
            "type": "adaptive_clock_start",
            "time_model": model_name,
            "config": self.config
        });

        self.span_emitter.emit_span(
            "adaptive_clock_start",
            "temporal_system",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de parada do clock
    async fn emit_clock_stop_span(&self) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "adaptive_clock_stop",
            "final_time_state": self.get_current_time_state()
        });

        self.span_emitter.emit_span(
            "adaptive_clock_stop",
            "temporal_system",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de tick temporal
    async fn emit_temporal_tick_span(&self, tick_result: &TemporalTickResult) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "temporal_tick",
            "time_state": tick_result.time_state,
            "events_triggered": tick_result.events_triggered.len(),
            "status_changed": tick_result.status_changed
        });

        self.span_emitter.emit_span(
            "temporal_tick",
            "temporal_system",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }
}

impl Clone for AdaptiveClock {
    fn clone(&self) -> Self {
        Self {
            id_with_keys: self.id_with_keys.clone(),
            span_emitter: Arc::clone(&self.span_emitter),
            time_system: Arc::clone(&self.time_system),
            config: self.config.clone(),
            temporal_listeners: Arc::clone(&self.temporal_listeners),
            scheduled_events: Arc::clone(&self.scheduled_events),
            is_running: Arc::clone(&self.is_running),
        }
    }
}

impl Default for AdaptiveClockConfig {
    fn default() -> Self {
        Self {
            base_tick_interval_ms: 1000, // 1 segundo base
            acceleration_factor: 1.0,
            pause_on_non_business_hours: false,
            emit_temporal_spans: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motor::span::SpanEmitter;
    use crate::enforcement::contextual_enforcer::{TimeUnit, TimeCalculationRule};

    #[tokio::test]
    async fn test_adaptive_clock_creation() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let config = AdaptiveClockConfig::default();
        
        let clock = AdaptiveClock::new(id_with_keys, span_emitter, config);
        assert!(!*clock.is_running.lock().unwrap());
    }

    #[tokio::test]
    async fn test_time_model_loading() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let config = AdaptiveClockConfig::default();
        let clock = AdaptiveClock::new(id_with_keys, span_emitter, config);
        
        let time_model = TimeModel {
            name: "test_model".to_string(),
            unit: TimeUnit::Hours,
            business_calendar: None,
            calculation_rules: vec![],
        };
        
        let result = clock.load_time_model(time_model);
        assert!(result.is_ok());
        
        let time_state = clock.get_current_time_state();
        assert!(time_state.current_timestamp > 0);
    }

    #[tokio::test]
    async fn test_event_scheduling() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let config = AdaptiveClockConfig::default();
        let clock = AdaptiveClock::new(id_with_keys, span_emitter, config);
        
        let event = TemporalEvent {
            event_id: "test_event".to_string(),
            event_type: crate::time::time_model::TemporalEventType::Deadline,
            scheduled_for: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64 + 1000000, // 1 segundo no futuro
            scheduled_for_unit: 1.0,
            context: serde_json::json!({"test": true}),
            recurring: None,
        };
        
        let result = clock.schedule_event(event);
        assert!(result.is_ok());
        
        let events = clock.scheduled_events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "test_event");
    }
}