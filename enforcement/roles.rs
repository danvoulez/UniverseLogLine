use crate::timeline::Span;
use crate::rules::EnforcementAction;
use std::collections::{HashMap, HashSet};
use serde_json::Value;

/// Enforcer baseado em papéis (roles)
pub struct RoleBasedEnforcer {
    /// Mapeamento de identidades para papéis
    identity_roles: HashMap<String, HashSet<String>>,
    
    /// Identidade atual
    current_identity: Option<String>,
    
    /// Permite operações se não houver requisitos específicos de papel
    permissive_default: bool,
}

impl RoleBasedEnforcer {
    /// Cria um novo enforcer baseado em papéis
    pub fn new() -> Self {
        Self {
            identity_roles: HashMap::new(),
            current_identity: None,
            permissive_default: false,
        }
    }
    
    /// Define a identidade e papéis atuais
    pub fn set_current_identity(&mut self, identity: &str, roles: Vec<&str>) {
        let roles_set: HashSet<String> = roles.iter().map(|&r| r.to_string()).collect();
        self.identity_roles.insert(identity.to_string(), roles_set);
        self.current_identity = Some(identity.to_string());
    }
    
    /// Define a política padrão para spans sem requisitos de papel
    pub fn set_permissive_default(&mut self, permissive: bool) {
        self.permissive_default = permissive;
    }
    
    /// Adiciona um papel a uma identidade
    pub fn add_role(&mut self, identity: &str, role: &str) {
        self.identity_roles
            .entry(identity.to_string())
            .or_insert_with(HashSet::new)
            .insert(role.to_string());
    }
    
    /// Remove um papel de uma identidade
    pub fn remove_role(&mut self, identity: &str, role: &str) {
        if let Some(roles) = self.identity_roles.get_mut(identity) {
            roles.remove(role);
        }
    }
    
    /// Verifica se uma identidade tem um papel específico
    pub fn has_role(&self, identity: &str, role: &str) -> bool {
        self.identity_roles
            .get(identity)
            .map(|roles| roles.contains(role))
            .unwrap_or(false)
    }
    
    /// Aplica regras baseadas em papel a um span
    pub fn enforce(&self, span: &Span) -> EnforcementAction {
        // Verificar se há uma identidade atual
        let identity = match &self.current_identity {
            Some(id) => id,
            None => return EnforcementAction::Reject("Nenhuma identidade autenticada".to_string()),
        };
        
        // Extrair requisitos de papel do span
        let required_roles = self.extract_required_roles(span);
        
        // Se não há requisitos específicos, usar política padrão
        if required_roles.is_empty() {
            return if self.permissive_default {
                EnforcementAction::Allow
            } else {
                EnforcementAction::Reject("Span não especifica requisitos de papel".to_string())
            };
        }
        
        // Verificar se a identidade tem todos os papéis necessários
        let user_roles = match self.identity_roles.get(identity) {
            Some(roles) => roles,
            None => return EnforcementAction::Reject("Identidade sem papéis definidos".to_string()),
        };
        
        for role in &required_roles {
            if !user_roles.contains(role) {
                return EnforcementAction::Reject(
                    format!("Falta o papel '{}' necessário para esta operação", role)
                );
            }
        }
        
        // Todos os requisitos de papel foram atendidos
        EnforcementAction::Allow
    }
    
    /// Extrai os papéis necessários dos dados do span
    fn extract_required_roles(&self, span: &Span) -> Vec<String> {
        let mut required_roles = Vec::new();
        
        if let Some(data) = &span.data {
            // Tentar extrair papéis dos dados como JSON
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Verificar dentro do payload
                if let Some(payload) = json.get("payload") {
                    self.extract_roles_from_json(payload, &mut required_roles);
                } else {
                    // Verificar diretamente nos dados
                    self.extract_roles_from_json(&json, &mut required_roles);
                }
            }
        }
        
        required_roles
    }
    
    /// Extrai papéis de um objeto JSON
    fn extract_roles_from_json(&self, json: &Value, roles: &mut Vec<String>) {
        // Verificar campo "roles_required"
        if let Some(required) = json.get("roles_required") {
            if let Some(required_array) = required.as_array() {
                for role in required_array {
                    if let Some(role_str) = role.as_str() {
                        roles.push(role_str.to_string());
                    }
                }
            } else if let Some(role_str) = required.as_str() {
                roles.push(role_str.to_string());
            }
        }
        
        // Verificar campo "permissions"
        if let Some(permissions) = json.get("permissions") {
            if let Some(permissions_obj) = permissions.as_object() {
                if let Some(required) = permissions_obj.get("requires_roles") {
                    if let Some(required_array) = required.as_array() {
                        for role in required_array {
                            if let Some(role_str) = role.as_str() {
                                roles.push(role_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
}