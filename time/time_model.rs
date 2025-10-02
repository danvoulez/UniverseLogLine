/// # LogLine Time Model - Tempo Computável por Gramática
///
/// Sistema de tempo declarativo baseado na gramática local.
/// Substitui o tick fixo global por modelos computáveis específicos
/// de cada projeto/domínio.
///
/// Cada gramática declara seu próprio time_model com:
/// - Unidade de tempo (dias úteis, horas, ciclos, slots)
/// - Calendário de negócio (feriados, turnos, horários)
/// - Fórmulas de cálculo (vencimento, diferença, atraso)
/// - Rules de agendamento e triggers temporais
///
/// O tempo deixa de ser um relógio global e passa a ser
/// uma declaração computável no arquivo .lll de cada projeto.

use std::collections::HashMap;
use chrono::{DateTime, Utc, NaiveDate, Weekday, Duration as ChronoDuration};
use serde::{Serialize, Deserialize};

use crate::enforcement::contextual_enforcer::{TimeModel, TimeUnit, BusinessCalendar, TimeCalculationRule};

/// Sistema de tempo adaptativo baseado em gramática
pub struct AdaptiveTimeSystem {
    /// Modelo de tempo ativo carregado da gramática
    active_time_model: Option<TimeModel>,
    
    /// Cache de cálculos temporais
    calculation_cache: HashMap<String, CalculationResult>,
    
    /// Estado atual do tempo no modelo
    current_time_state: TimeState,
}

/// Estado temporal no contexto do modelo ativo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeState {
    /// Timestamp atual em microssegundos UTC
    pub current_timestamp: u64,
    
    /// Valor atual na unidade do modelo (ex: dia útil 245)
    pub current_unit_value: f64,
    
    /// Última atualização do estado
    pub last_updated: u64,
    
    /// Status do clock temporal
    pub clock_status: TemporalClockStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalClockStatus {
    /// Clock funcionando normalmente
    Running,
    /// Pausado (ex: fora do horário comercial)
    Paused,
    /// Feriado ou período não útil
    Holiday,
    /// Erro no cálculo temporal
    Error(String),
}

/// Resultado de um cálculo temporal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationResult {
    pub calculation_name: String,
    pub input_values: HashMap<String, serde_json::Value>,
    pub result: f64,
    pub unit: String,
    pub calculated_at: u64,
    pub expires_at: Option<u64>,
}

/// Evento temporal agendado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEvent {
    pub event_id: String,
    pub event_type: TemporalEventType,
    pub scheduled_for: u64, // timestamp UTC
    pub scheduled_for_unit: f64, // valor na unidade do modelo
    pub context: serde_json::Value,
    pub recurring: Option<RecurrencePattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalEventType {
    /// Deadline/vencimento
    Deadline,
    /// Trigger automático
    AutoTrigger, 
    /// Agendamento de tarefa
    ScheduledTask,
    /// Checkpoint temporal
    Checkpoint,
    /// Notificação
    Notification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurrencePattern {
    pub pattern_type: RecurrenceType,
    pub interval: u32,
    pub end_condition: Option<RecurrenceEnd>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecurrenceType {
    /// A cada N unidades do modelo
    Every(u32),
    /// Dias específicos da semana
    WeekDays(Vec<Weekday>),
    /// Dias específicos do mês
    MonthDays(Vec<u8>),
    /// Datas específicas
    SpecificDates(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecurrenceEnd {
    /// Para depois de N ocorrências
    AfterOccurrences(u32),
    /// Para em data específica
    EndDate(u64),
    /// Para quando condição for atendida
    Condition(String),
}

impl AdaptiveTimeSystem {
    /// Cria novo sistema de tempo sem modelo carregado
    pub fn new() -> Self {
        Self {
            active_time_model: None,
            calculation_cache: HashMap::new(),
            current_time_state: TimeState {
                current_timestamp: Self::current_timestamp_micros(),
                current_unit_value: 0.0,
                last_updated: Self::current_timestamp_micros(),
                clock_status: TemporalClockStatus::Running,
            },
        }
    }

    /// Carrega modelo de tempo da gramática
    pub fn load_time_model(&mut self, time_model: TimeModel) -> Result<(), Box<dyn std::error::Error>> {
        println!("⏰ Carregando modelo de tempo: {} ({})", time_model.name, time_model.unit.as_str());
        
        // Valida modelo
        self.validate_time_model(&time_model)?;
        
        // Inicializa estado baseado no modelo
        self.initialize_time_state(&time_model)?;
        
        self.active_time_model = Some(time_model);
        
        println!("✅ Modelo de tempo carregado. Unidade: {}, Valor atual: {:.2}", 
            self.active_time_model.as_ref().unwrap().unit.as_str(),
            self.current_time_state.current_unit_value
        );
        
        Ok(())
    }

    /// Calcula valor temporal usando fórmula da gramática
    pub fn calculate(
        &mut self, 
        calculation_name: &str,
        inputs: HashMap<String, serde_json::Value>
    ) -> Result<CalculationResult, Box<dyn std::error::Error>> {
        let time_model = self.active_time_model.as_ref()
            .ok_or("Nenhum modelo de tempo carregado")?;

        // Procura regra de cálculo
        let rule = time_model.calculation_rules.iter()
            .find(|r| r.name == calculation_name)
            .ok_or(format!("Regra de cálculo '{}' não encontrada", calculation_name))?;

        // Executa cálculo
        let result = self.execute_calculation_rule(rule, &inputs)?;
        
        let calculation_result = CalculationResult {
            calculation_name: calculation_name.to_string(),
            input_values: inputs,
            result,
            unit: time_model.unit.as_str().to_string(),
            calculated_at: Self::current_timestamp_micros(),
            expires_at: None, // Cache não expira por padrão
        };

        // Cache resultado
        self.calculation_cache.insert(calculation_name.to_string(), calculation_result.clone());

        Ok(calculation_result)
    }

    /// Converte timestamp UTC para valor na unidade do modelo
    pub fn timestamp_to_model_units(&self, timestamp: u64) -> Result<f64, Box<dyn std::error::Error>> {
        let time_model = self.active_time_model.as_ref()
            .ok_or("Nenhum modelo de tempo carregado")?;

        match &time_model.unit {
            TimeUnit::Microseconds => Ok(timestamp as f64),
            TimeUnit::Days => {
                // Converte microssegundos para dias
                Ok(timestamp as f64 / (24.0 * 60.0 * 60.0 * 1_000_000.0))
            },
            TimeUnit::BusinessDays => {
                self.calculate_business_days_from_timestamp(timestamp)
            },
            TimeUnit::Hours => {
                Ok(timestamp as f64 / (60.0 * 60.0 * 1_000_000.0))
            },
            TimeUnit::Cycles => {
                // Ciclos customizados - depende da gramática
                self.calculate_custom_cycles(timestamp)
            },
            TimeUnit::Slots => {
                // Slots de tempo - ex: slots de 30min para TV
                self.calculate_time_slots(timestamp)
            },
            TimeUnit::Weeks => {
                Ok(timestamp as f64 / (7.0 * 24.0 * 60.0 * 60.0 * 1_000_000.0))
            },
        }
    }

    /// Converte valor na unidade do modelo para timestamp UTC
    pub fn model_units_to_timestamp(&self, unit_value: f64) -> Result<u64, Box<dyn std::error::Error>> {
        let time_model = self.active_time_model.as_ref()
            .ok_or("Nenhum modelo de tempo carregado")?;

        match &time_model.unit {
            TimeUnit::Microseconds => Ok(unit_value as u64),
            TimeUnit::Days => {
                Ok((unit_value * 24.0 * 60.0 * 60.0 * 1_000_000.0) as u64)
            },
            TimeUnit::BusinessDays => {
                self.calculate_timestamp_from_business_days(unit_value)
            },
            TimeUnit::Hours => {
                Ok((unit_value * 60.0 * 60.0 * 1_000_000.0) as u64)
            },
            TimeUnit::Cycles => {
                self.calculate_timestamp_from_cycles(unit_value)
            },
            TimeUnit::Slots => {
                self.calculate_timestamp_from_slots(unit_value)
            },
            TimeUnit::Weeks => {
                Ok((unit_value * 7.0 * 24.0 * 60.0 * 60.0 * 1_000_000.0) as u64)
            },
        }
    }

    /// Atualiza estado temporal
    pub fn tick(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let current_time = Self::current_timestamp_micros();
        
        if let Some(time_model) = &self.active_time_model {
            // Atualiza valor na unidade do modelo
            let new_unit_value = self.timestamp_to_model_units(current_time)?;
            
            // Verifica se deve pausar (ex: fora do horário comercial)
            let should_pause = self.should_pause_clock(current_time)?;
            
            self.current_time_state = TimeState {
                current_timestamp: current_time,
                current_unit_value: new_unit_value,
                last_updated: current_time,
                clock_status: if should_pause {
                    TemporalClockStatus::Paused
                } else {
                    TemporalClockStatus::Running
                },
            };
        }

        Ok(())
    }

    /// Verifica se algum evento temporal deve ser disparado
    pub fn check_temporal_events(&self, events: &[TemporalEvent]) -> Vec<TemporalEvent> {
        let mut triggered_events = Vec::new();
        let current_time = self.current_time_state.current_timestamp;
        let current_unit = self.current_time_state.current_unit_value;

        for event in events {
            if self.should_trigger_event(event, current_time, current_unit) {
                triggered_events.push(event.clone());
            }
        }

        triggered_events
    }

    /// Valida modelo de tempo
    fn validate_time_model(&self, time_model: &TimeModel) -> Result<(), Box<dyn std::error::Error>> {
        if time_model.name.is_empty() {
            return Err("Modelo de tempo deve ter nome".into());
        }

        // Valida regras de cálculo
        for rule in &time_model.calculation_rules {
            if rule.name.is_empty() || rule.formula.is_empty() {
                return Err("Regras de cálculo devem ter nome e fórmula".into());
            }
        }

        Ok(())
    }

    /// Inicializa estado temporal baseado no modelo
    fn initialize_time_state(&mut self, time_model: &TimeModel) -> Result<(), Box<dyn std::error::Error>> {
        let current_time = Self::current_timestamp_micros();
        let current_unit_value = self.timestamp_to_model_units(current_time)?;

        self.current_time_state = TimeState {
            current_timestamp: current_time,
            current_unit_value,
            last_updated: current_time,
            clock_status: TemporalClockStatus::Running,
        };

        Ok(())
    }

    /// Executa regra de cálculo
    fn execute_calculation_rule(
        &self,
        rule: &TimeCalculationRule,
        inputs: &HashMap<String, serde_json::Value>
    ) -> Result<f64, Box<dyn std::error::Error>> {
        // Parser simples de fórmulas - em implementação real seria um parser completo
        match rule.formula.as_str() {
            "current_time" => {
                Ok(self.current_time_state.current_unit_value)
            },
            formula if formula.contains("created_at + prazo_dias") => {
                let created_at = inputs.get("created_at")
                    .and_then(|v| v.as_u64())
                    .ok_or("created_at obrigatório para cálculo de vencimento")?;
                    
                let prazo_dias = inputs.get("prazo_dias")
                    .and_then(|v| v.as_f64())
                    .ok_or("prazo_dias obrigatório para cálculo de vencimento")?;

                let created_unit = self.timestamp_to_model_units(created_at)?;
                Ok(created_unit + prazo_dias)
            },
            formula if formula.contains("current_time - created_at") => {
                let created_at = inputs.get("created_at")
                    .and_then(|v| v.as_u64())
                    .ok_or("created_at obrigatório para cálculo de idade")?;

                let created_unit = self.timestamp_to_model_units(created_at)?;
                Ok(self.current_time_state.current_unit_value - created_unit)
            },
            _ => {
                println!("⚠️ Fórmula não implementada no parser simples: {}", rule.formula);
                Ok(0.0)
            }
        }
    }

    /// Calcula dias úteis a partir do timestamp
    fn calculate_business_days_from_timestamp(&self, timestamp: u64) -> Result<f64, Box<dyn std::error::Error>> {
        let time_model = self.active_time_model.as_ref().unwrap();
        
        if let Some(calendar) = &time_model.business_calendar {
            // Implementação simplificada - considera apenas work_days
            let days_since_epoch = timestamp as f64 / (24.0 * 60.0 * 60.0 * 1_000_000.0);
            let business_days = days_since_epoch * (calendar.work_days.len() as f64 / 7.0);
            Ok(business_days)
        } else {
            // Sem calendário - assume 5 dias úteis por semana
            let days_since_epoch = timestamp as f64 / (24.0 * 60.0 * 60.0 * 1_000_000.0);
            Ok(days_since_epoch * (5.0 / 7.0))
        }
    }

    /// Calcula timestamp a partir de dias úteis
    fn calculate_timestamp_from_business_days(&self, business_days: f64) -> Result<u64, Box<dyn std::error::Error>> {
        let time_model = self.active_time_model.as_ref().unwrap();
        
        if let Some(calendar) = &time_model.business_calendar {
            let calendar_days = business_days * (7.0 / calendar.work_days.len() as f64);
            Ok((calendar_days * 24.0 * 60.0 * 60.0 * 1_000_000.0) as u64)
        } else {
            // Sem calendário - assume 5 dias úteis por semana
            let calendar_days = business_days * (7.0 / 5.0);
            Ok((calendar_days * 24.0 * 60.0 * 60.0 * 1_000_000.0) as u64)
        }
    }

    /// Calcula ciclos customizados
    fn calculate_custom_cycles(&self, timestamp: u64) -> Result<f64, Box<dyn std::error::Error>> {
        // Implementação depende da definição na gramática
        // Por exemplo: ciclos de 4 horas
        let cycle_duration_micros = 4.0 * 60.0 * 60.0 * 1_000_000.0;
        Ok(timestamp as f64 / cycle_duration_micros)
    }

    /// Calcula timestamp a partir de ciclos
    fn calculate_timestamp_from_cycles(&self, cycles: f64) -> Result<u64, Box<dyn std::error::Error>> {
        let cycle_duration_micros = 4.0 * 60.0 * 60.0 * 1_000_000.0;
        Ok((cycles * cycle_duration_micros) as u64)
    }

    /// Calcula slots de tempo
    fn calculate_time_slots(&self, timestamp: u64) -> Result<f64, Box<dyn std::error::Error>> {
        // Slots de 30 minutos para TV
        let slot_duration_micros = 30.0 * 60.0 * 1_000_000.0;
        Ok(timestamp as f64 / slot_duration_micros)
    }

    /// Calcula timestamp a partir de slots
    fn calculate_timestamp_from_slots(&self, slots: f64) -> Result<u64, Box<dyn std::error::Error>> {
        let slot_duration_micros = 30.0 * 60.0 * 1_000_000.0;
        Ok((slots * slot_duration_micros) as u64)
    }

    /// Verifica se clock deve pausar
    fn should_pause_clock(&self, current_time: u64) -> Result<bool, Box<dyn std::error::Error>> {
        let time_model = self.active_time_model.as_ref().unwrap();
        
        if let Some(calendar) = &time_model.business_calendar {
            // Implementação simplificada - verifica apenas horário comercial
            let dt = DateTime::from_timestamp((current_time / 1_000_000) as i64, 0)
                .ok_or("Timestamp inválido")?;
                
            let hour = dt.hour() as u8;
            let is_work_hour = hour >= calendar.work_hours.0 && hour < calendar.work_hours.1;
            
            Ok(!is_work_hour)
        } else {
            Ok(false) // Não pausa se não há calendário
        }
    }

    /// Verifica se evento deve ser disparado
    fn should_trigger_event(&self, event: &TemporalEvent, current_time: u64, current_unit: f64) -> bool {
        // Verifica se chegou o momento
        if current_time >= event.scheduled_for && current_unit >= event.scheduled_for_unit {
            return true;
        }

        false
    }

    /// Retorna timestamp atual em microssegundos
    fn current_timestamp_micros() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Retorna estado atual do tempo
    pub fn get_time_state(&self) -> &TimeState {
        &self.current_time_state
    }

    /// Retorna modelo de tempo ativo
    pub fn get_active_model(&self) -> Option<&TimeModel> {
        self.active_time_model.as_ref()
    }
}

impl TimeUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeUnit::Microseconds => "microseconds",
            TimeUnit::Days => "days", 
            TimeUnit::BusinessDays => "business_days",
            TimeUnit::Hours => "hours",
            TimeUnit::Cycles => "cycles",
            TimeUnit::Slots => "slots", 
            TimeUnit::Weeks => "weeks",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_time_system_creation() {
        let time_system = AdaptiveTimeSystem::new();
        assert!(time_system.active_time_model.is_none());
        assert_eq!(time_system.calculation_cache.len(), 0);
    }

    #[test]
    fn test_time_model_loading() {
        let mut time_system = AdaptiveTimeSystem::new();
        
        let time_model = TimeModel {
            name: "business_days".to_string(),
            unit: TimeUnit::BusinessDays,
            business_calendar: Some(BusinessCalendar {
                holidays: vec!["2025-12-25".to_string()],
                work_days: vec![1, 2, 3, 4, 5], // Segunda a sexta
                work_hours: (9, 18), // 9h às 18h
            }),
            calculation_rules: vec![
                TimeCalculationRule {
                    name: "vencimento".to_string(),
                    formula: "created_at + prazo_dias".to_string(),
                }
            ],
        };
        
        let result = time_system.load_time_model(time_model);
        assert!(result.is_ok());
        assert!(time_system.active_time_model.is_some());
    }

    #[test]
    fn test_timestamp_conversion() {
        let mut time_system = AdaptiveTimeSystem::new();
        
        let time_model = TimeModel {
            name: "days".to_string(),
            unit: TimeUnit::Days,
            business_calendar: None,
            calculation_rules: vec![],
        };
        
        time_system.load_time_model(time_model).unwrap();
        
        // Testa conversão de dias
        let one_day_micros = 24 * 60 * 60 * 1_000_000; // 1 dia em microssegundos
        let days = time_system.timestamp_to_model_units(one_day_micros).unwrap();
        assert!((days - 1.0).abs() < 0.001); // ~1 dia
        
        let timestamp = time_system.model_units_to_timestamp(1.0).unwrap();
        assert_eq!(timestamp, one_day_micros);
    }

    #[test]
    fn test_calculation_execution() {
        let mut time_system = AdaptiveTimeSystem::new();
        
        let time_model = TimeModel {
            name: "test".to_string(),
            unit: TimeUnit::Days,
            business_calendar: None,
            calculation_rules: vec![
                TimeCalculationRule {
                    name: "vencimento".to_string(),
                    formula: "created_at + prazo_dias".to_string(),
                }
            ],
        };
        
        time_system.load_time_model(time_model).unwrap();
        
        let mut inputs = HashMap::new();
        inputs.insert("created_at".to_string(), serde_json::Value::Number(serde_json::Number::from(0u64)));
        inputs.insert("prazo_dias".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(30.0).unwrap()));
        
        let result = time_system.calculate("vencimento", inputs).unwrap();
        assert_eq!(result.calculation_name, "vencimento");
        assert!((result.result - 30.0).abs() < 0.001);
    }
}