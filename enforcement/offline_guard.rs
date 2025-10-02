use crate::timeline::Span;
use crate::rules::EnforcementAction;
use crate::enforcement::Enforcer;
use crate::enforcement::roles::RoleBasedEnforcer;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use ed25519_dalek::{Verifier, PublicKey};
use serde_json::Value;

/// Políticas para quando a fila de offline está cheia
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueueFullPolicy {
    /// Rejeita novos spans
    RejectNew,
    
    /// Remove spans mais antigos para dar espaço aos novos
    RemoveOldest,
    
    /// Falha com erro
    Error,
}

/// Guarda que verifica assinaturas offline nos spans e gerencia modo offline
pub struct OfflineGuard {
    /// Chaves públicas conhecidas por identidade
    public_keys: HashMap<String, PublicKey>,
    
    /// Enforcer baseado em papéis usado como backup
    role_enforcer: Option<RoleBasedEnforcer>,
    
    /// Fila de spans para processar quando voltar online
    pending_spans: Arc<Mutex<VecDeque<Span>>>,
    
    /// Capacidade máxima da fila
    max_capacity: usize,
    
    /// Política para quando a fila está cheia
    queue_full_policy: QueueFullPolicy,
    
    /// Indica se o sistema está em modo offline
    is_offline: Arc<Mutex<bool>>,
}

impl OfflineGuard {
    /// Cria um novo guarda offline
    pub fn new(max_capacity: usize, policy: QueueFullPolicy) -> Self {
        Self {
            public_keys: HashMap::new(),
            role_enforcer: None,
            pending_spans: Arc::new(Mutex::new(VecDeque::with_capacity(max_capacity))),
            max_capacity,
            queue_full_policy: policy,
            is_offline: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Define o enforcer de papéis de backup
    pub fn with_role_enforcer(mut self, enforcer: RoleBasedEnforcer) -> Self {
        self.role_enforcer = Some(enforcer);
        self
    }
    
    /// Define o estado offline
    pub fn set_offline_mode(&self, offline: bool) {
        let mut mode = self.is_offline.lock().unwrap();
        *mode = offline;
    }
    
    /// Verifica se está em modo offline
    pub fn is_offline(&self) -> bool {
        let mode = self.is_offline.lock().unwrap();
        *mode
    }
    
    /// Registra uma chave pública para uma identidade
    pub fn register_public_key(&mut self, identity: &str, public_key_bytes: &[u8]) -> Result<(), String> {
        match PublicKey::from_bytes(public_key_bytes) {
            Ok(key) => {
                self.public_keys.insert(identity.to_string(), key);
                Ok(())
            },
            Err(e) => Err(format!("Chave pública inválida: {}", e)),
        }
    }
    
    /// Obtém a chave pública para uma identidade
    fn get_public_key(&self, identity: &str) -> Option<&PublicKey> {
        self.public_keys.get(identity)
    }
    
    /// Extrai a identidade do assinante do span
    fn extract_signer_identity(&self, span: &Span) -> Option<String> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Verificar campo de assinatura
                if let Some(signature) = json.get("signature") {
                    // Verificar identidade do assinante
                    if let Some(identity) = signature.get("identity")
                        .and_then(|i| i.as_str()) {
                        return Some(identity.to_string());
                    }
                }
                
                // Verificar no payload
                if let Some(payload) = json.get("payload") {
                    if let Some(signature) = payload.get("signature") {
                        if let Some(identity) = signature.get("identity")
                            .and_then(|i| i.as_str()) {
                            return Some(identity.to_string());
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// Extrai dados assinados do span
    fn extract_signed_data(&self, span: &Span) -> Option<Vec<u8>> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Tentar extrair campo "signed_data"
                if let Some(signed_data) = json.get("signed_data") {
                    if let Some(data_str) = signed_data.as_str() {
                        return base64::decode(data_str).ok();
                    }
                }
                
                // Tentar extrair do campo "payload"
                if let Some(payload) = json.get("payload") {
                    if let Some(signed_data) = payload.get("signed_data") {
                        if let Some(data_str) = signed_data.as_str() {
                            return base64::decode(data_str).ok();
                        }
                    }
                }
                
                // Se não encontrou dados assinados específicos, usar todo o conteúdo (exceto signature)
                if let Some(mut data_copy) = json.as_object().cloned() {
                    // Remover campo de assinatura
                    data_copy.remove("signature");
                    
                    // Serializar para bytes
                    if let Ok(serialized) = serde_json::to_string(&data_copy) {
                        return Some(serialized.into_bytes());
                    }
                }
            }
        }
        
        None
    }
    
    /// Extrai assinatura do span
    fn extract_signature(&self, span: &Span) -> Option<Vec<u8>> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Verificar campo de assinatura
                if let Some(signature) = json.get("signature") {
                    if let Some(sig_data) = signature.get("data")
                        .and_then(|s| s.as_str()) {
                        return base64::decode(sig_data).ok();
                    }
                }
                
                // Verificar no payload
                if let Some(payload) = json.get("payload") {
                    if let Some(signature) = payload.get("signature") {
                        if let Some(sig_data) = signature.get("data")
                            .and_then(|s| s.as_str()) {
                            return base64::decode(sig_data).ok();
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// Verifica uma assinatura
    fn verify_signature(&self, public_key: &PublicKey, signature: &[u8], data: &[u8]) -> bool {
        // Converter assinatura para formato ed25519
        if signature.len() != 64 {
            return false;
        }
        
        // Criar objeto Signature do ed25519_dalek
        match ed25519_dalek::Signature::from_bytes(signature) {
            Ok(sig) => {
                // Verificar assinatura
                public_key.verify(data, &sig).is_ok()
            },
            Err(_) => false,
        }
    }
    
    /// Processa um span em modo offline
    pub fn process_offline(&self, span: Span) -> Result<EnforcementAction, String> {
        // Validação básica mesmo offline
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Verificar se tem informações mínimas
                if json.as_object().map_or(true, |obj| obj.is_empty()) {
                    return Err("Dados do span estão vazios".to_string());
                }
            } else {
                return Err("Dados do span não estão em formato JSON válido".to_string());
            }
        } else {
            return Err("Span não contém dados".to_string());
        }
        
        // Adicionar span à fila de pendentes
        let mut pending = self.pending_spans.lock().unwrap();
        
        // Verificar capacidade
        if pending.len() >= self.max_capacity {
            match self.queue_full_policy {
                QueueFullPolicy::RejectNew => {
                    return Err("Fila de spans offline está cheia, rejeitando novo span".to_string());
                },
                QueueFullPolicy::RemoveOldest => {
                    // Remover o span mais antigo
                    pending.pop_front();
                },
                QueueFullPolicy::Error => {
                    return Err("Fila de spans offline está cheia, erro crítico".to_string());
                }
            }
        }
        
        // Adicionar span à fila
        pending.push_back(span);
        
        // Em modo offline, apenas simulamos o span
        Ok(EnforcementAction::SimulateOnly)
    }
    
    /// Processa todos os spans pendentes quando o sistema volta online
    pub fn process_pending_spans<F>(&self, online_processor: F) -> Vec<Result<(), String>>
    where
        F: Fn(&Span) -> Result<(), String>
    {
        let mut results = Vec::new();
        let mut pending = self.pending_spans.lock().unwrap();
        
        // Processar cada span pendente
        while let Some(span) = pending.pop_front() {
            let result = online_processor(&span);
            results.push(result);
        }
        
        results
    }
    
    /// Retorna o número de spans pendentes
    pub fn pending_count(&self) -> usize {
        let pending = self.pending_spans.lock().unwrap();
        pending.len()
    }
    
    /// Limpa todos os spans pendentes
    pub fn clear_pending(&self) {
        let mut pending = self.pending_spans.lock().unwrap();
        pending.clear();
    }
}

impl Enforcer for OfflineGuard {
    fn enforce(&self, span: &Span) -> EnforcementAction {
        // Verificar se estamos em modo offline
        if self.is_offline() {
            if let Ok(action) = self.process_offline(span.clone()) {
                return action;
            } else {
                return EnforcementAction::Reject("Não foi possível processar o span em modo offline".to_string());
            }
        }
        
        // Verificar se o span requer verificação de assinatura
        // Spans sem dados são permitidos (podem ser operações administrativas)
        if span.data.is_none() {
            return EnforcementAction::Allow;
        }
        
        // Extrair identidade do assinante
        let identity = match self.extract_signer_identity(span) {
            Some(id) => id,
            None => {
                // Se não há identidade de assinante, tentar usar enforcer de papéis
                if let Some(role_enforcer) = &self.role_enforcer {
                    return role_enforcer.enforce(span);
                } else {
                    return EnforcementAction::Reject("Identidade do assinante não encontrada".to_string());
                }
            }
        };
        
        // Obter chave pública
        let public_key = match self.get_public_key(&identity) {
            Some(key) => key,
            None => return EnforcementAction::Reject("Chave pública não encontrada para o assinante".to_string()),
        };
        
        // Extrair dados assinados
        let signed_data = match self.extract_signed_data(span) {
            Some(data) => data,
            None => return EnforcementAction::Reject("Dados assinados não encontrados".to_string()),
        };
        
        // Extrair assinatura
        let signature = match self.extract_signature(span) {
            Some(sig) => sig,
            None => return EnforcementAction::Reject("Assinatura não encontrada".to_string()),
        };
        
        // Verificar assinatura
        if self.verify_signature(public_key, &signature, &signed_data) {
            EnforcementAction::Allow
        } else {
            EnforcementAction::Reject("Assinatura inválida".to_string())
        }
    }
}