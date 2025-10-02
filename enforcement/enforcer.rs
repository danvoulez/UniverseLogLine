use crate::timeline::Span;
use crate::rules::EnforcementAction;
use std::sync::Arc;

/// Trait para componentes de enforcement
pub trait Enforcer {
    /// Aplica regras de enforcement a um span
    fn enforce(&self, span: &Span) -> EnforcementAction;
}

/// Enforcer composto que combina múltiplos enforcers
pub struct CompositeEnforcer {
    enforcers: Vec<Arc<dyn Enforcer + Send + Sync>>,
    require_all: bool,
}

impl CompositeEnforcer {
    /// Cria um novo enforcer composto
    pub fn new(require_all: bool) -> Self {
        Self {
            enforcers: Vec::new(),
            require_all,
        }
    }
    
    /// Adiciona um enforcer à composição
    pub fn add_enforcer(&mut self, enforcer: Arc<dyn Enforcer + Send + Sync>) {
        self.enforcers.push(enforcer);
    }
}

impl Enforcer for CompositeEnforcer {
    fn enforce(&self, span: &Span) -> EnforcementAction {
        if self.enforcers.is_empty() {
            return EnforcementAction::Allow;
        }
        
        if self.require_all {
            // Todos os enforcers devem permitir
            for enforcer in &self.enforcers {
                match enforcer.enforce(span) {
                    EnforcementAction::Allow => continue,
                    reject @ EnforcementAction::Reject(_) => return reject,
                }
            }
            EnforcementAction::Allow
        } else {
            // Pelo menos um enforcer deve permitir
            let mut last_rejection = EnforcementAction::Reject("Nenhum enforcer disponível".to_string());
            
            for enforcer in &self.enforcers {
                match enforcer.enforce(span) {
                    EnforcementAction::Allow => return EnforcementAction::Allow,
                    reject @ EnforcementAction::Reject(_) => last_rejection = reject,
                }
            }
            
            // Se chegou aqui, nenhum permitiu
            last_rejection
        }
    }
}

/// Enforcer que sempre permite
pub struct AllowAllEnforcer;

impl Enforcer for AllowAllEnforcer {
    fn enforce(&self, _span: &Span) -> EnforcementAction {
        EnforcementAction::Allow
    }
}

/// Enforcer que aplica regras específicas por canal
pub struct ChannelEnforcer {
    channel_rules: std::collections::HashMap<String, Arc<dyn Enforcer + Send + Sync>>,
    default_enforcer: Arc<dyn Enforcer + Send + Sync>,
}

impl ChannelEnforcer {
    /// Cria um novo enforcer baseado em canais
    pub fn new(default_enforcer: Arc<dyn Enforcer + Send + Sync>) -> Self {
        Self {
            channel_rules: std::collections::HashMap::new(),
            default_enforcer,
        }
    }
    
    /// Define um enforcer para um canal específico
    pub fn set_channel_enforcer(&mut self, channel: &str, enforcer: Arc<dyn Enforcer + Send + Sync>) {
        self.channel_rules.insert(channel.to_string(), enforcer);
    }
}

impl Enforcer for ChannelEnforcer {
    fn enforce(&self, span: &Span) -> EnforcementAction {
        if let Some(enforcer) = self.channel_rules.get(&span.channel) {
            enforcer.enforce(span)
        } else {
            self.default_enforcer.enforce(span)
        }
    }
}