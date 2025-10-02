//! Módulo de enforcement de regras para LogLine
//! 
//! Este módulo fornece componentes para aplicar regras e políticas
//! de segurança em spans na timeline. Inclui enforcer baseado em papéis,
//! validadores, enforcer contextual e guardas offline.

// Re-export principais componentes
pub use self::enforcer::{Enforcer, CompositeEnforcer, AllowAllEnforcer, ChannelEnforcer};
pub use self::roles::RoleBasedEnforcer;
pub use self::validator::SpanValidator;
pub use self::contextual_enforcer::ContextualEnforcer;
pub use self::offline_guard::OfflineGuard;
pub use self::hierarchy::HierarchyEnforcer;

// Módulos internos
pub mod enforcer;
pub mod roles;
pub mod validator;
pub mod contextual_enforcer;
pub mod offline_guard;
pub mod hierarchy;
pub mod audit;

/// Utilidades para enforcement de regras
pub mod utils {
    use crate::timeline::Span;
    use serde_json::{Value, json};
    
    /// Extrai um valor de um span usando um caminho de acesso
    pub fn extract_value_from_span(span: &Span, path: &str) -> Option<Value> {
        if let Some(data) = &span.data {
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                let parts: Vec<&str> = path.split('.').collect();
                let mut current = &json;
                
                for part in &parts {
                    if let Some(next) = current.get(part) {
                        current = next;
                    } else {
                        return None;
                    }
                }
                
                return Some(current.clone());
            }
        }
        
        None
    }
    
    /// Adiciona metadados de enforcement a um span
    pub fn add_enforcement_metadata(span: &mut Span, metadata: Value) -> Result<(), String> {
        let data = match &span.data {
            Some(data) => {
                let mut json: Value = serde_json::from_str(data)
                    .map_err(|e| format!("Erro ao analisar dados do span: {}", e))?;
                
                // Adicionar ou atualizar seção de metadata
                let metadata_section = json.get_mut("metadata")
                    .and_then(|m| m.as_object_mut());
                
                if let Some(meta_obj) = metadata_section {
                    // Atualizar seção existente
                    if let Some(enforcement) = meta_obj.get_mut("enforcement") {
                        if let Some(enforcement_obj) = enforcement.as_object_mut() {
                            // Mesclar com dados existentes
                            if let Some(metadata_obj) = metadata.as_object() {
                                for (key, value) in metadata_obj {
                                    enforcement_obj.insert(key.clone(), value.clone());
                                }
                            }
                        } else {
                            *enforcement = metadata;
                        }
                    } else {
                        // Adicionar nova entrada de enforcement
                        meta_obj.insert("enforcement".to_string(), metadata);
                    }
                } else {
                    // Criar nova seção de metadata
                    json["metadata"] = json!({
                        "enforcement": metadata
                    });
                }
                
                serde_json::to_string(&json)
                    .map_err(|e| format!("Erro ao serializar dados do span: {}", e))?
            },
            None => {
                // Criar dados do zero
                let json = json!({
                    "metadata": {
                        "enforcement": metadata
                    }
                });
                
                serde_json::to_string(&json)
                    .map_err(|e| format!("Erro ao serializar dados do span: {}", e))?
            }
        };
        
        span.data = Some(data);
        Ok(())
    }
}