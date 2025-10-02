use crate::timeline::Span;
use crate::rules::EnforcementAction;
use crate::enforcement::Enforcer;
use serde_json::Value;
use std::collections::HashMap;

/// Enforcer baseado em hierarquia organizacional
pub struct HierarchyEnforcer {
    /// Estrutura hierárquica: mapa de entidade para seu superior
    hierarchy: HashMap<String, String>,
    
    /// Mapa de identidade para entidade
    identity_entity: HashMap<String, String>,
    
    /// Identidade atual
    current_identity: Option<String>,
}

impl HierarchyEnforcer {
    /// Cria um novo enforcer hierárquico
    pub fn new() -> Self {
        Self {
            hierarchy: HashMap::new(),
            identity_entity: HashMap::new(),
            current_identity: None,
        }
    }
    
    /// Define a identidade atual
    pub fn set_current_identity(&mut self, identity: &str) {
        self.current_identity = Some(identity.to_string());
    }
    
    /// Mapeia uma identidade para uma entidade
    pub fn map_identity_to_entity(&mut self, identity: &str, entity: &str) {
        self.identity_entity.insert(identity.to_string(), entity.to_string());
    }
    
    /// Define uma relação hierárquica: child é subordinado a parent
    pub fn set_hierarchy_relation(&mut self, child: &str, parent: &str) {
        self.hierarchy.insert(child.to_string(), parent.to_string());
    }
    
    /// Verifica se entity_a está acima de entity_b na hierarquia
    pub fn is_superior_to(&self, entity_a: &str, entity_b: &str) -> bool {
        if entity_a == entity_b {
            return true; // Uma entidade é considerada superior a si mesma
        }
        
        let mut current = entity_b;
        while let Some(superior) = self.hierarchy.get(current) {
            if superior == entity_a {
                return true;
            }
            current = superior;
        }
        
        false
    }
    
    /// Extrai a entidade proprietária de um span
    fn extract_span_owner(&self, span: &Span) -> Option<String> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Verificar campo owner/entity/creator
                if let Some(owner) = json.get("owner")
                    .and_then(|o| o.as_str()) {
                    return Some(owner.to_string());
                }
                
                if let Some(entity) = json.get("entity")
                    .and_then(|e| e.as_str()) {
                    return Some(entity.to_string());
                }
                
                if let Some(creator) = json.get("creator")
                    .and_then(|c| c.as_str()) {
                    return Some(creator.to_string());
                }
                
                // Verificar dentro do payload
                if let Some(payload) = json.get("payload") {
                    if let Some(owner) = payload.get("owner")
                        .and_then(|o| o.as_str()) {
                        return Some(owner.to_string());
                    }
                    
                    if let Some(entity) = payload.get("entity")
                        .and_then(|e| e.as_str()) {
                        return Some(entity.to_string());
                    }
                }
            }
        }
        
        None
    }
    
    /// Determina a operação que está sendo realizada no span
    fn determine_operation(&self, span: &Span) -> Option<String> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                // Verificar campo operation/action
                if let Some(op) = json.get("operation")
                    .and_then(|o| o.as_str()) {
                    return Some(op.to_string());
                }
                
                if let Some(action) = json.get("action")
                    .and_then(|a| a.as_str()) {
                    return Some(action.to_string());
                }
                
                // Verificar tipo como operação
                if let Some(typ) = json.get("type")
                    .and_then(|t| t.as_str()) {
                    return Some(typ.to_string());
                }
                
                // Verificar dentro do payload
                if let Some(payload) = json.get("payload") {
                    if let Some(op) = payload.get("operation")
                        .and_then(|o| o.as_str()) {
                        return Some(op.to_string());
                    }
                    
                    if let Some(action) = payload.get("action")
                        .and_then(|a| a.as_str()) {
                        return Some(action.to_string());
                    }
                    
                    if let Some(typ) = payload.get("type")
                        .and_then(|t| t.as_str()) {
                        return Some(typ.to_string());
                    }
                }
            }
        }
        
        None
    }
    
    /// Verifica se uma operação pode ser realizada com base na hierarquia
    fn can_perform_operation(&self, operator_entity: &str, target_entity: &str, operation: &str) -> bool {
        // Operações permitidas apenas para a própria entidade ou superiores
        let restricted_ops = [
            "delete", "remove", "cancel", "approve", "reject", "update_status",
            "change_permissions", "assign", "transfer_ownership",
        ];
        
        // Verificar se a operação é restrita
        if restricted_ops.contains(&operation) {
            return self.is_superior_to(operator_entity, target_entity);
        }
        
        // Para outras operações, verificar casos específicos
        match operation {
            "read" | "view" => true, // Leitura é permitida para todos
            "create" => true,        // Qualquer um pode criar
            "update" | "edit" => {
                // Edição requer ser a mesma entidade ou superior
                self.is_superior_to(operator_entity, target_entity)
            },
            _ => {
                // Para operações desconhecidas, permitir apenas se for a mesma entidade ou superior
                self.is_superior_to(operator_entity, target_entity)
            }
        }
    }
}

impl Enforcer for HierarchyEnforcer {
    fn enforce(&self, span: &Span) -> EnforcementAction {
        // Verificar se há uma identidade atual
        let current_identity = match &self.current_identity {
            Some(id) => id,
            None => return EnforcementAction::Reject("Nenhuma identidade autenticada".to_string()),
        };
        
        // Obter a entidade da identidade atual
        let operator_entity = match self.identity_entity.get(current_identity) {
            Some(entity) => entity,
            None => return EnforcementAction::Reject("Identidade não mapeada para uma entidade".to_string()),
        };
        
        // Extrair a entidade proprietária do span
        let target_entity = match self.extract_span_owner(span) {
            Some(entity) => entity,
            None => return EnforcementAction::Allow, // Se não há proprietário, permitir
        };
        
        // Determinar a operação sendo realizada
        let operation = match self.determine_operation(span) {
            Some(op) => op,
            None => "unknown".to_string(), // Operação desconhecida é tratada como restrita
        };
        
        // Verificar se a operação pode ser realizada
        if self.can_perform_operation(operator_entity, &target_entity, &operation) {
            EnforcementAction::Allow
        } else {
            EnforcementAction::Reject(format!(
                "Operação '{}' não permitida na hierarquia atual", operation
            ))
        }
    }
}