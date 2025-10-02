use crate::timeline::Span;
use crate::rules::EnforcementAction;
use serde_json::Value;
use std::collections::HashMap;
use regex::Regex;

/// Validator para verificar integridade e consistência de spans
pub struct SpanValidator {
    /// Validadores registrados por tipo de span
    validators: HashMap<String, Vec<Box<dyn Fn(&Span) -> Result<(), String>>>>,
    
    /// Validadores que se aplicam a todos os spans
    global_validators: Vec<Box<dyn Fn(&Span) -> Result<(), String>>>,
    
    /// Indica se deve falhar rapidamente nas validações
    fail_fast: bool,
}

impl SpanValidator {
    /// Cria um novo validador de spans
    pub fn new(fail_fast: bool) -> Self {
        Self {
            validators: HashMap::new(),
            global_validators: Vec::new(),
            fail_fast,
        }
    }
    
    /// Adiciona um validador para um tipo específico de span
    pub fn add_validator<F>(&mut self, span_type: &str, validator: F) 
    where
        F: Fn(&Span) -> Result<(), String> + 'static,
    {
        self.validators
            .entry(span_type.to_string())
            .or_insert_with(Vec::new)
            .push(Box::new(validator));
    }
    
    /// Adiciona um validador que se aplica a todos os tipos de span
    pub fn add_global_validator<F>(&mut self, validator: F)
    where
        F: Fn(&Span) -> Result<(), String> + 'static,
    {
        self.global_validators.push(Box::new(validator));
    }
    
    /// Valida um span e retorna uma ação de enforcement
    pub fn validate(&self, span: &Span) -> EnforcementAction {
        let mut errors = Vec::new();
        
        // Executar validadores globais
        for validator in &self.global_validators {
            if let Err(message) = validator(span) {
                errors.push(message);
                if self.fail_fast {
                    return EnforcementAction::Reject(message);
                }
            }
        }
        
        // Extrair o tipo do span
        let span_type = self.extract_span_type(span);
        
        // Executar validadores específicos para o tipo de span
        if let Some(span_type) = span_type {
            if let Some(validators) = self.validators.get(&span_type) {
                for validator in validators {
                    if let Err(message) = validator(span) {
                        errors.push(message);
                        if self.fail_fast {
                            return EnforcementAction::Reject(message);
                        }
                    }
                }
            }
        }
        
        // Retornar resultado final
        if errors.is_empty() {
            EnforcementAction::Allow
        } else {
            EnforcementAction::Reject(errors.join("; "))
        }
    }
    
    /// Extrai o tipo de span dos seus dados
    fn extract_span_type(&self, span: &Span) -> Option<String> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Tentar extrair o tipo a partir da estrutura de dados
                if let Some(t) = json.get("type")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()) {
                    return Some(t);
                }
                
                if let Some(t) = json.get("span_type")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()) {
                    return Some(t);
                }
                
                // Tentar extrair o tipo do payload
                if let Some(payload) = json.get("payload") {
                    if let Some(t) = payload.get("type")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string()) {
                        return Some(t);
                    }
                }
            }
        }
        
        // Fallback para nome do canal como tipo
        span.channel.clone()
    }
}

/// Funções utilitárias para criar validadores comuns
pub mod validators {
    use super::*;
    
    /// Cria um validador que verifica a presença de um campo obrigatório
    pub fn required_field(field_path: &str) -> impl Fn(&Span) -> Result<(), String> {
        let field_path = field_path.to_string();
        move |span| {
            if let Some(data) = &span.data {
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    let path_parts: Vec<&str> = field_path.split('.').collect();
                    let mut current = &json;
                    
                    for part in &path_parts {
                        if let Some(next) = current.get(part) {
                            current = next;
                        } else {
                            return Err(format!("Campo obrigatório '{}' não encontrado", field_path));
                        }
                    }
                    
                    // Verificar se o valor não é nulo ou vazio
                    if current.is_null() || 
                       (current.is_string() && current.as_str().unwrap().is_empty()) ||
                       (current.is_array() && current.as_array().unwrap().is_empty()) {
                        return Err(format!("Campo obrigatório '{}' está vazio", field_path));
                    }
                    
                    Ok(())
                } else {
                    Err("Dados do span não estão em formato JSON válido".to_string())
                }
            } else {
                Err("Span não contém dados".to_string())
            }
        }
    }
    
    /// Cria um validador que verifica se um campo corresponde a um formato regex
    pub fn pattern_match(field_path: &str, pattern: &str) -> impl Fn(&Span) -> Result<(), String> {
        let field_path = field_path.to_string();
        let regex = Regex::new(pattern).expect("Padrão regex inválido");
        
        move |span| {
            if let Some(data) = &span.data {
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    let path_parts: Vec<&str> = field_path.split('.').collect();
                    let mut current = &json;
                    
                    for part in &path_parts {
                        if let Some(next) = current.get(part) {
                            current = next;
                        } else {
                            return Ok(()); // Campo não encontrado, validador não se aplica
                        }
                    }
                    
                    if let Some(value_str) = current.as_str() {
                        if regex.is_match(value_str) {
                            Ok(())
                        } else {
                            Err(format!("Campo '{}' não corresponde ao padrão esperado", field_path))
                        }
                    } else {
                        Err(format!("Campo '{}' não é uma string", field_path))
                    }
                } else {
                    Err("Dados do span não estão em formato JSON válido".to_string())
                }
            } else {
                Ok(()) // Sem dados, validador não se aplica
            }
        }
    }
    
    /// Cria um validador que verifica se um campo numérico está dentro de um intervalo
    pub fn numeric_range(field_path: &str, min: Option<f64>, max: Option<f64>) -> impl Fn(&Span) -> Result<(), String> {
        let field_path = field_path.to_string();
        
        move |span| {
            if let Some(data) = &span.data {
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    let path_parts: Vec<&str> = field_path.split('.').collect();
                    let mut current = &json;
                    
                    for part in &path_parts {
                        if let Some(next) = current.get(part) {
                            current = next;
                        } else {
                            return Ok(()); // Campo não encontrado, validador não se aplica
                        }
                    }
                    
                    if let Some(value) = current.as_f64() {
                        if let Some(min_val) = min {
                            if value < min_val {
                                return Err(format!("Campo '{}' é menor que o mínimo permitido", field_path));
                            }
                        }
                        
                        if let Some(max_val) = max {
                            if value > max_val {
                                return Err(format!("Campo '{}' é maior que o máximo permitido", field_path));
                            }
                        }
                        
                        Ok(())
                    } else {
                        Err(format!("Campo '{}' não é um número", field_path))
                    }
                } else {
                    Err("Dados do span não estão em formato JSON válido".to_string())
                }
            } else {
                Ok(()) // Sem dados, validador não se aplica
            }
        }
    }
}