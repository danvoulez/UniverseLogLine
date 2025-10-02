//! Multi-Tenant Role Management System
//! Extends the role system to handle tenant-aware permissions and hierarchies

use crate::enforcement::{Role, Agent, Permissions, EnforcementError, EnforcementResult as Result};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Extended agent with multi-tenant context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTenantAgent {
    pub logline_id: String,
    pub role: Role,
    pub permissions: Permissions,
    
    // Tenant context
    pub tenant_id: Option<String>,
    pub organization_id: Option<Uuid>,
    pub organization_role: OrganizationRole,
    pub organization_permissions: OrganizationPermissions,
    
    // Hierarchy and delegation
    pub parent_id: Option<String>,
    pub delegated_from: Option<String>,
    pub delegation_expires_at: Option<DateTime<Utc>>,
    
    // Cross-tenant access
    pub cross_tenant_access: HashMap<String, CrossTenantAccess>,
    
    // Audit
    pub created_at: DateTime<Utc>,
    pub last_validation: Option<DateTime<Utc>>,
    pub status: AgentStatus,
}

/// Organization-specific roles
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrganizationRole {
    Owner,          // Full organization control
    Admin,          // Administrative privileges
    Manager,        // Team/department management
    Member,         // Standard organization member
    Contributor,    // Limited contribution access
    Viewer,         // Read-only access
    Guest,          // Temporary limited access
    System,         // System/service account
}

/// Organization-level permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationPermissions {
    // User management
    pub can_invite_users: bool,
    pub can_remove_users: bool,
    pub can_manage_roles: bool,
    
    // Organization management
    pub can_manage_settings: bool,
    pub can_manage_branding: bool,
    pub can_manage_policies: bool,
    
    // Data access
    pub can_access_all_spans: bool,
    pub can_create_organization_spans: bool,
    pub can_manage_organization_data: bool,
    
    // Audit and compliance
    pub can_access_audit_logs: bool,
    pub can_export_data: bool,
    pub can_manage_compliance: bool,
    
    // Federation
    pub can_federate: bool,
    pub can_manage_federation: bool,
    pub can_cross_tenant_access: bool,
    
    // Ghost identities
    pub can_create_ghost_identities: bool,
    pub can_manage_ghost_identities: bool,
    
    // Advanced features
    pub can_use_advanced_features: bool,
    pub can_manage_api_keys: bool,
    pub can_manage_webhooks: bool,
}

/// Cross-tenant access permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossTenantAccess {
    pub tenant_id: String,
    pub role: OrganizationRole,
    pub permissions: Vec<String>,
    pub granted_at: DateTime<Utc>,
    pub granted_by: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Agent status in multi-tenant context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Active,         // Fully active
    Pending,        // Pending activation (e.g., invitation not accepted)
    Suspended,      // Temporarily suspended
    Archived,       // Archived but not deleted
    Revoked,        // Access revoked
}

impl OrganizationPermissions {
    /// Create permissions for a specific organization role
    pub fn for_organization_role(role: &OrganizationRole) -> Self {
        match role {
            OrganizationRole::Owner => OrganizationPermissions {
                can_invite_users: true,
                can_remove_users: true,
                can_manage_roles: true,
                can_manage_settings: true,
                can_manage_branding: true,
                can_manage_policies: true,
                can_access_all_spans: true,
                can_create_organization_spans: true,
                can_manage_organization_data: true,
                can_access_audit_logs: true,
                can_export_data: true,
                can_manage_compliance: true,
                can_federate: true,
                can_manage_federation: true,
                can_cross_tenant_access: true,
                can_create_ghost_identities: true,
                can_manage_ghost_identities: true,
                can_use_advanced_features: true,
                can_manage_api_keys: true,
                can_manage_webhooks: true,
            },
            OrganizationRole::Admin => OrganizationPermissions {
                can_invite_users: true,
                can_remove_users: true,
                can_manage_roles: true,
                can_manage_settings: true,
                can_manage_branding: false,
                can_manage_policies: false,
                can_access_all_spans: true,
                can_create_organization_spans: true,
                can_manage_organization_data: true,
                can_access_audit_logs: true,
                can_export_data: true,
                can_manage_compliance: false,
                can_federate: false,
                can_manage_federation: false,
                can_cross_tenant_access: false,
                can_create_ghost_identities: true,
                can_manage_ghost_identities: true,
                can_use_advanced_features: true,
                can_manage_api_keys: true,
                can_manage_webhooks: false,
            },
            OrganizationRole::Manager => OrganizationPermissions {
                can_invite_users: true,
                can_remove_users: false,
                can_manage_roles: false,
                can_manage_settings: false,
                can_manage_branding: false,
                can_manage_policies: false,
                can_access_all_spans: false,
                can_create_organization_spans: true,
                can_manage_organization_data: false,
                can_access_audit_logs: false,
                can_export_data: false,
                can_manage_compliance: false,
                can_federate: false,
                can_manage_federation: false,
                can_cross_tenant_access: false,
                can_create_ghost_identities: false,
                can_manage_ghost_identities: false,
                can_use_advanced_features: false,
                can_manage_api_keys: false,
                can_manage_webhooks: false,
            },
            OrganizationRole::Member => OrganizationPermissions {
                can_invite_users: false,
                can_remove_users: false,
                can_manage_roles: false,
                can_manage_settings: false,
                can_manage_branding: false,
                can_manage_policies: false,
                can_access_all_spans: false,
                can_create_organization_spans: true,
                can_manage_organization_data: false,
                can_access_audit_logs: false,
                can_export_data: false,
                can_manage_compliance: false,
                can_federate: false,
                can_manage_federation: false,
                can_cross_tenant_access: false,
                can_create_ghost_identities: false,
                can_manage_ghost_identities: false,
                can_use_advanced_features: false,
                can_manage_api_keys: false,
                can_manage_webhooks: false,
            },
            OrganizationRole::Contributor => OrganizationPermissions {
                can_invite_users: false,
                can_remove_users: false,
                can_manage_roles: false,
                can_manage_settings: false,
                can_manage_branding: false,
                can_manage_policies: false,
                can_access_all_spans: false,
                can_create_organization_spans: false,
                can_manage_organization_data: false,
                can_access_audit_logs: false,
                can_export_data: false,
                can_manage_compliance: false,
                can_federate: false,
                can_manage_federation: false,
                can_cross_tenant_access: false,
                can_create_ghost_identities: false,
                can_manage_ghost_identities: false,
                can_use_advanced_features: false,
                can_manage_api_keys: false,
                can_manage_webhooks: false,
            },
            OrganizationRole::Viewer => OrganizationPermissions::none(),
            OrganizationRole::Guest => OrganizationPermissions::none(),
            OrganizationRole::System => OrganizationPermissions {
                can_invite_users: false,
                can_remove_users: false,
                can_manage_roles: false,
                can_manage_settings: false,
                can_manage_branding: false,
                can_manage_policies: false,
                can_access_all_spans: true,
                can_create_organization_spans: true,
                can_manage_organization_data: true,
                can_access_audit_logs: true,
                can_export_data: true,
                can_manage_compliance: false,
                can_federate: true,
                can_manage_federation: false,
                can_cross_tenant_access: false,
                can_create_ghost_identities: true,
                can_manage_ghost_identities: true,
                can_use_advanced_features: true,
                can_manage_api_keys: false,
                can_manage_webhooks: false,
            },
        }
    }
    
    /// Create empty permissions
    pub fn none() -> Self {
        OrganizationPermissions {
            can_invite_users: false,
            can_remove_users: false,
            can_manage_roles: false,
            can_manage_settings: false,
            can_manage_branding: false,
            can_manage_policies: false,
            can_access_all_spans: false,
            can_create_organization_spans: false,
            can_manage_organization_data: false,
            can_access_audit_logs: false,
            can_export_data: false,
            can_manage_compliance: false,
            can_federate: false,
            can_manage_federation: false,
            can_cross_tenant_access: false,
            can_create_ghost_identities: false,
            can_manage_ghost_identities: false,
            can_use_advanced_features: false,
            can_manage_api_keys: false,
            can_manage_webhooks: false,
        }
    }
}

/// Multi-tenant roles manager
pub struct MultiTenantRolesManager {
    agents: HashMap<String, MultiTenantAgent>,
    config_path: std::path::PathBuf,
}

impl MultiTenantRolesManager {
    pub fn new() -> Result<Self> {
        let mut config_path = dirs::home_dir().ok_or_else(|| {
            EnforcementError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Home directory not found"
            ))
        })?;
        config_path.push(".logline");
        config_path.push("multi_tenant_roles.json");
        
        Ok(MultiTenantRolesManager {
            agents: HashMap::new(),
            config_path,
        })
    }
    
    /// Load roles from configuration file
    pub fn load_roles(&mut self) -> Result<()> {
        if !self.config_path.exists() {
            return Ok(());
        }
        
        let content = std::fs::read_to_string(&self.config_path)?;
        let agents: HashMap<String, MultiTenantAgent> = serde_json::from_str(&content)?;
        self.agents = agents;
        
        Ok(())
    }
    
    /// Save roles to configuration file
    pub fn save_roles(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(&self.agents)?;
        std::fs::write(&self.config_path, content)?;
        
        Ok(())
    }
    
    /// Add an agent with multi-tenant context
    pub fn add_agent(
        &mut self,
        logline_id: String,
        role: Role,
        tenant_id: Option<String>,
        organization_id: Option<Uuid>,
        organization_role: OrganizationRole,
        parent_id: Option<String>
    ) -> Result<()> {
        let agent = MultiTenantAgent {
            logline_id: logline_id.clone(),
            role: role.clone(),
            permissions: Permissions::for_role(&role),
            tenant_id,
            organization_id,
            organization_role: organization_role.clone(),
            organization_permissions: OrganizationPermissions::for_organization_role(&organization_role),
            parent_id,
            delegated_from: None,
            delegation_expires_at: None,
            cross_tenant_access: HashMap::new(),
            created_at: Utc::now(),
            last_validation: None,
            status: AgentStatus::Active,
        };
        
        self.agents.insert(logline_id, agent);
        self.save_roles()?;
        
        Ok(())
    }
    
    /// Get agent with tenant filtering
    pub fn get_agent(&self, logline_id: &str, context_tenant_id: Option<&str>) -> Option<&MultiTenantAgent> {
        let agent = self.agents.get(logline_id)?;
        
        // Check if agent has access in this tenant context
        match (context_tenant_id, &agent.tenant_id) {
            (None, None) => Some(agent), // Both individual
            (Some(ctx_tenant), Some(agent_tenant)) if ctx_tenant == agent_tenant => Some(agent), // Same tenant
            (Some(ctx_tenant), _) => {
                // Check cross-tenant access
                if agent.cross_tenant_access.contains_key(ctx_tenant) {
                    Some(agent)
                } else {
                    None
                }
            },
            _ => None,
        }
    }
    
    /// List agents for a specific tenant
    pub fn list_agents_for_tenant(&self, tenant_id: Option<&str>) -> Vec<&MultiTenantAgent> {
        self.agents.values()
            .filter(|agent| {
                match (tenant_id, &agent.tenant_id) {
                    (None, None) => true,
                    (Some(t1), Some(t2)) if t1 == t2 => true,
                    (Some(ctx_tenant), _) => agent.cross_tenant_access.contains_key(ctx_tenant),
                    _ => false,
                }
            })
            .collect()
    }
    
    /// Grant cross-tenant access
    pub fn grant_cross_tenant_access(
        &mut self,
        logline_id: &str,
        target_tenant_id: String,
        role: OrganizationRole,
        granted_by: String,
        expires_at: Option<DateTime<Utc>>
    ) -> Result<()> {
        if let Some(agent) = self.agents.get_mut(logline_id) {
            let access = CrossTenantAccess {
                tenant_id: target_tenant_id.clone(),
                role: role.clone(),
                permissions: vec![], // Could be customized
                granted_at: Utc::now(),
                granted_by,
                expires_at,
            };
            
            agent.cross_tenant_access.insert(target_tenant_id, access);
            agent.last_validation = Some(Utc::now());
            self.save_roles()?;
        }
        
        Ok(())
    }
    
    /// Revoke cross-tenant access
    pub fn revoke_cross_tenant_access(&mut self, logline_id: &str, target_tenant_id: &str) -> Result<()> {
        if let Some(agent) = self.agents.get_mut(logline_id) {
            agent.cross_tenant_access.remove(target_tenant_id);
            agent.last_validation = Some(Utc::now());
            self.save_roles()?;
        }
        
        Ok(())
    }
    
    /// Update agent status
    pub fn update_agent_status(&mut self, logline_id: &str, status: AgentStatus) -> Result<()> {
        if let Some(agent) = self.agents.get_mut(logline_id) {
            agent.status = status;
            agent.last_validation = Some(Utc::now());
            self.save_roles()?;
        }
        
        Ok(())
    }
    
    /// Check if agent has specific organization permission in tenant context
    pub fn has_organization_permission(
        &self,
        logline_id: &str,
        tenant_id: Option<&str>,
        permission: &str
    ) -> bool {
        if let Some(agent) = self.get_agent(logline_id, tenant_id) {
            if agent.status != AgentStatus::Active {
                return false;
            }
            
            // Check organization permissions
            match permission {
                "can_invite_users" => agent.organization_permissions.can_invite_users,
                "can_remove_users" => agent.organization_permissions.can_remove_users,
                "can_manage_roles" => agent.organization_permissions.can_manage_roles,
                "can_manage_settings" => agent.organization_permissions.can_manage_settings,
                "can_access_audit_logs" => agent.organization_permissions.can_access_audit_logs,
                "can_create_ghost_identities" => agent.organization_permissions.can_create_ghost_identities,
                "can_federate" => agent.organization_permissions.can_federate,
                _ => false,
            }
        } else {
            false
        }
    }
    
    /// Clean up expired delegations and cross-tenant access
    pub fn cleanup_expired_access(&mut self) -> Result<()> {
        let now = Utc::now();
        let mut modified = false;
        
        for agent in self.agents.values_mut() {
            // Clean up expired delegations
            if let Some(expires_at) = agent.delegation_expires_at {
                if now > expires_at {
                    agent.delegated_from = None;
                    agent.delegation_expires_at = None;
                    modified = true;
                }
            }
            
            // Clean up expired cross-tenant access
            let expired_tenants: Vec<String> = agent.cross_tenant_access
                .iter()
                .filter(|(_, access)| {
                    if let Some(expires_at) = access.expires_at {
                        now > expires_at
                    } else {
                        false
                    }
                })
                .map(|(tenant_id, _)| tenant_id.clone())
                .collect();
            
            for tenant_id in expired_tenants {
                agent.cross_tenant_access.remove(&tenant_id);
                modified = true;
            }
        }
        
        if modified {
            self.save_roles()?;
        }
        
        Ok(())
    }
}