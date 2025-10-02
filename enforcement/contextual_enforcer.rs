use crate::timeline::{Span, Timeline};
use crate::rules::EnforcementAction;
use crate::enforcement::Enforcer;
use std::sync::{Arc, Mutex};

/// Enforcer que mantém contexto de spans anteriores para avaliar regras
pub struct ContextualEnforcer {
    /// Referência ao timeline para consulta de spans anteriores
    timeline: Arc<Mutex<dyn Timeline>>,
    
    /// Tamanho da janela de contexto (quantos spans anteriores considerar)
    context_window: usize,
    
    /// Cache de spans já processados por este enforcer
    #[allow(dead_code)]
    processed_spans: Mutex<Vec<String>>,
}

impl ContextualEnforcer {
    /// Cria um novo enforcer contextual
    pub fn new(timeline: Arc<Mutex<dyn Timeline>>, context_window: usize) -> Self {
        Self {
            timeline,
            context_window,
            processed_spans: Mutex::new(Vec::new()),
        }
    }
    
    /// Obtém spans anteriores no mesmo canal
    fn get_context_spans(&self, span: &Span) -> Vec<Span> {
        if let Ok(timeline) = self.timeline.lock() {
            match timeline.query(
                Some(&span.channel),
                None, // Sem limites de timestamp
                Some(self.context_window),
            ) {
                Ok(spans) => spans,
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }
    
    /// Verifica se um span é uma transição válida do estado atual
    fn is_valid_transition(&self, current_span: &Span, context_spans: &[Span]) -> bool {
        // Identificar o tipo do span atual
        let current_type = match self.extract_span_type(current_span) {
            Some(t) => t,
            None => return true, // Se não conseguirmos determinar o tipo, permitimos
        };
        
        // Para alguns tipos de span, podemos ter regras específicas de transição
        match current_type.as_str() {
            "contract_execution" => self.validate_contract_execution(current_span, context_spans),
            "payment" => self.validate_payment(current_span, context_spans),
            "document_update" => self.validate_document_update(current_span, context_spans),
            "state_transition" => self.validate_state_transition(current_span, context_spans),
            _ => true, // Para outros tipos, permitimos por padrão
        }
    }
    
    /// Extrai o tipo do span a partir dos seus dados
    fn extract_span_type(&self, span: &Span) -> Option<String> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // Tentar extrair diretamente
                if let Some(t) = json.get("type")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()) {
                    return Some(t);
                }
                
                // Tentar dentro do payload
                if let Some(payload) = json.get("payload") {
                    if let Some(t) = payload.get("type")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string()) {
                        return Some(t);
                    }
                }
            }
        }
        
        // Fallback para o nome do canal como tipo
        Some(span.channel.clone())
    }
    
    /// Valida execução de contrato no contexto
    fn validate_contract_execution(&self, span: &Span, context: &[Span]) -> bool {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // Extrair ID do contrato
                if let Some(contract_id) = json.get("contract_id")
                    .and_then(|id| id.as_str()) {
                    
                    // Verificar se existe um span de criação de contrato correspondente
                    let contract_exists = context.iter().any(|s| {
                        if let Some(s_data) = &s.data {
                            if let Ok(s_json) = serde_json::from_str::<serde_json::Value>(s_data) {
                                // Verificar se é um span de criação de contrato
                                let is_contract_creation = s_json.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| t == "contract_creation")
                                    .unwrap_or(false);
                                
                                // Verificar se o ID corresponde
                                let ids_match = s_json.get("contract_id")
                                    .and_then(|id| id.as_str())
                                    .map(|id| id == contract_id)
                                    .unwrap_or(false);
                                
                                return is_contract_creation && ids_match;
                            }
                        }
                        false
                    });
                    
                    return contract_exists;
                }
            }
        }
        
        // Se não conseguirmos validar, assumimos que é válido
        true
    }
    
    /// Valida pagamento no contexto
    fn validate_payment(&self, span: &Span, context: &[Span]) -> bool {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // Extrair ID da fatura relacionada ao pagamento
                if let Some(invoice_id) = json.get("invoice_id")
                    .and_then(|id| id.as_str()) {
                    
                    // Verificar se existe um span de fatura correspondente
                    let invoice_exists = context.iter().any(|s| {
                        if let Some(s_data) = &s.data {
                            if let Ok(s_json) = serde_json::from_str::<serde_json::Value>(s_data) {
                                // Verificar se é um span de fatura
                                let is_invoice = s_json.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| t == "invoice")
                                    .unwrap_or(false);
                                
                                // Verificar se o ID corresponde
                                let ids_match = s_json.get("id")
                                    .and_then(|id| id.as_str())
                                    .map(|id| id == invoice_id)
                                    .unwrap_or(false);
                                
                                return is_invoice && ids_match;
                            }
                        }
                        false
                    });
                    
                    // Verificar se esta fatura já foi paga anteriormente
                    let already_paid = context.iter().any(|s| {
                        if let Some(s_data) = &s.data {
                            if let Ok(s_json) = serde_json::from_str::<serde_json::Value>(s_data) {
                                // Verificar se é um span de pagamento
                                let is_payment = s_json.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| t == "payment")
                                    .unwrap_or(false);
                                
                                // Verificar se o ID da fatura corresponde
                                let ids_match = s_json.get("invoice_id")
                                    .and_then(|id| id.as_str())
                                    .map(|id| id == invoice_id)
                                    .unwrap_or(false);
                                
                                return is_payment && ids_match;
                            }
                        }
                        false
                    });
                    
                    return invoice_exists && !already_paid;
                }
            }
        }
        
        true
    }
    
    /// Valida atualização de documento no contexto
    fn validate_document_update(&self, span: &Span, context: &[Span]) -> bool {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // Extrair ID do documento
                if let Some(document_id) = json.get("document_id")
                    .and_then(|id| id.as_str()) {
                    
                    // Extrair versão do documento
                    let version = json.get("version")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    
                    // Para atualizações (version > 1), verificar se existe versão anterior
                    if version > 1 {
                        let previous_version_exists = context.iter().any(|s| {
                            if let Some(s_data) = &s.data {
                                if let Ok(s_json) = serde_json::from_str::<serde_json::Value>(s_data) {
                                    // Verificar se é um update ou criação de documento
                                    let is_doc_span = s_json.get("type")
                                        .and_then(|t| t.as_str())
                                        .map(|t| t == "document_update" || t == "document_creation")
                                        .unwrap_or(false);
                                    
                                    // Verificar se o ID do documento corresponde
                                    let ids_match = s_json.get("document_id")
                                        .and_then(|id| id.as_str())
                                        .map(|id| id == document_id)
                                        .unwrap_or(false);
                                    
                                    // Verificar se é a versão anterior
                                    let is_previous_version = s_json.get("version")
                                        .and_then(|v| v.as_u64())
                                        .map(|v| v == version - 1)
                                        .unwrap_or(false);
                                    
                                    return is_doc_span && ids_match && is_previous_version;
                                }
                            }
                            false
                        });
                        
                        return previous_version_exists;
                    }
                }
            }
        }
        
        true
    }
    
    /// Valida transição de estado no contexto
    fn validate_state_transition(&self, span: &Span, context: &[Span]) -> bool {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                // Extrair o processo e estado atual
                let process = json.get("process_id").and_then(|p| p.as_str());
                let to_state = json.get("to_state").and_then(|s| s.as_str());
                
                if let (Some(process_id), Some(to_state)) = (process, to_state) {
                    // Encontrar o último estado do mesmo processo
                    let mut found_current = false;
                    let mut current_state = None;
                    
                    // Percorrer spans do mais recente para o mais antigo
                    for s in context.iter().rev() {
                        if let Some(s_data) = &s.data {
                            if let Ok(s_json) = serde_json::from_str::<serde_json::Value>(s_data) {
                                // Verificar se é uma transição de estado do mesmo processo
                                let is_state_transition = s_json.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| t == "state_transition")
                                    .unwrap_or(false);
                                
                                let process_matches = s_json.get("process_id")
                                    .and_then(|p| p.as_str())
                                    .map(|p| p == process_id)
                                    .unwrap_or(false);
                                
                                if is_state_transition && process_matches {
                                    // Encontramos uma transição anterior do mesmo processo
                                    found_current = true;
                                    
                                    // Capturar o estado para qual esta transição foi
                                    current_state = s_json.get("to_state")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string());
                                    
                                    break;
                                }
                            }
                        }
                    }
                    
                    if found_current {
                        // Verificar se o estado atual permite transição para o novo estado
                        // Aqui podemos implementar uma matriz de transição ou regras mais complexas
                        // Por enquanto, uma implementação simplificada:
                        
                        match (current_state.as_deref(), to_state) {
                            // Exemplo de regras de transição para um fluxo de pedido
                            (Some("draft"), "submitted") => true,
                            (Some("submitted"), "approved") => true,
                            (Some("submitted"), "rejected") => true,
                            (Some("approved"), "in_progress") => true,
                            (Some("in_progress"), "completed") => true,
                            
                            // Regras para um fluxo de pagamento
                            (Some("pending"), "processing") => true,
                            (Some("processing"), "completed") => true,
                            (Some("processing"), "failed") => true,
                            
                            // Regra genérica para iniciar um processo
                            (None, _) => true,
                            
                            // Transições não permitidas
                            _ => false,
                        }
                    } else {
                        // Se não encontramos estado atual, permitimos iniciar em qualquer estado
                        true
                    }
                } else {
                    // Dados incompletos
                    false
                }
            } else {
                // JSON inválido
                false
            }
        } else {
            // Sem dados
            false
        }
    }
}

impl Enforcer for ContextualEnforcer {
    fn enforce(&self, span: &Span) -> EnforcementAction {
        // Obter spans de contexto
        let context_spans = self.get_context_spans(span);
        
        // Validar transição
        if !self.is_valid_transition(span, &context_spans) {
            return EnforcementAction::Reject(
                "Transição de estado inválida no contexto atual".to_string()
            );
        }
        
        // Registrar este span como processado
        if let Ok(mut processed) = self.processed_spans.lock() {
            processed.push(span.id.clone());
            
            // Limitar o tamanho do cache
            if processed.len() > 1000 {
                processed.remove(0);
            }
        }
        
        EnforcementAction::Allow
    }
}