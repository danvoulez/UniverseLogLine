//! Multi-Tenant CLI Commands for LogLine ID
//! Provides tenant-aware operations for organization management

use clap::{Args, Subcommand};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Multi-tenant CLI commands
#[derive(Subcommand, Debug)]
pub enum MultiTenantCommand {
    /// Organization management
    #[command(subcommand)]
    Org(OrgCommand),
    
    /// Tenant operations
    #[command(subcommand)]
    Tenant(TenantCommand),
    
    /// Identity management in multi-tenant context
    #[command(subcommand)]
    Identity(IdentityCommand),
    
    /// Federation operations
    #[command(subcommand)]
    Federation(FederationCommand),
}

/// Organization-specific commands
#[derive(Subcommand, Debug)]
pub enum OrgCommand {
    /// Create a new organization
    Create {
        /// Organization name
        #[arg(long)]
        name: String,
        
        /// Organization domain (for email validation)
        #[arg(long)]
        domain: Option<String>,
        
        /// Subdomain for tenant isolation
        #[arg(long)]
        subdomain: Option<String>,
        
        /// Organization type
        #[arg(long, default_value = "standard")]
        org_type: String,
    },
    
    /// List organizations for current user
    List,
    
    /// Show organization details
    Show {
        /// Organization ID or subdomain
        org_id: String,
    },
    
    /// Update organization settings
    Update {
        /// Organization ID
        org_id: String,
        
        /// New name
        #[arg(long)]
        name: Option<String>,
        
        /// New domain
        #[arg(long)]
        domain: Option<String>,
        
        /// New subdomain
        #[arg(long)]
        subdomain: Option<String>,
    },
    
    /// Invite user to organization
    Invite {
        /// Organization ID
        org_id: String,
        
        /// User email or LogLine ID
        user: String,
        
        /// Role to assign
        #[arg(long, default_value = "member")]
        role: String,
        
        /// Send invitation email
        #[arg(long)]
        send_email: bool,
    },
    
    /// Remove user from organization
    Remove {
        /// Organization ID
        org_id: String,
        
        /// User LogLine ID
        user_id: String,
    },
    
    /// List organization members
    Members {
        /// Organization ID
        org_id: String,
    },
    
    /// Set organization branding
    Branding {
        /// Organization ID
        org_id: String,
        
        /// Logo URL
        #[arg(long)]
        logo: Option<String>,
        
        /// Primary color (hex)
        #[arg(long)]
        primary_color: Option<String>,
        
        /// Secondary color (hex)
        #[arg(long)]
        secondary_color: Option<String>,
        
        /// Custom CSS
        #[arg(long)]
        custom_css: Option<String>,
    },
}

/// Tenant-specific commands
#[derive(Subcommand, Debug)]
pub enum TenantCommand {
    /// Switch to a tenant context
    Switch {
        /// Tenant ID or subdomain
        tenant_id: String,
    },
    
    /// Show current tenant context
    Current,
    
    /// List available tenants for current user
    List,
    
    /// Create a span in tenant context
    CreateSpan {
        /// Span name
        name: String,
        
        /// Span type
        #[arg(long, default_value = "organization")]
        span_type: String,
        
        /// Private span (organization only)
        #[arg(long)]
        private: bool,
        
        /// Parent span ID
        #[arg(long)]
        parent: Option<String>,
    },
    
    /// List spans in tenant context
    ListSpans {
        /// Include private spans
        #[arg(long)]
        include_private: bool,
        
        /// Filter by type
        #[arg(long)]
        span_type: Option<String>,
    },
    
    /// Export tenant data
    Export {
        /// Export format (json, csv)
        #[arg(long, default_value = "json")]
        format: String,
        
        /// Output file
        #[arg(long)]
        output: Option<String>,
        
        /// Include audit logs
        #[arg(long)]
        include_audit: bool,
    },
}

/// Identity management commands in multi-tenant context
#[derive(Subcommand, Debug)]
pub enum IdentityCommand {
    /// Create organization-scoped identity
    CreateOrgIdentity {
        /// Organization ID
        org_id: String,
        
        /// Identity name
        name: String,
        
        /// Identity type (user, service, ghost)
        #[arg(long, default_value = "user")]
        identity_type: String,
        
        /// Role in organization
        #[arg(long, default_value = "member")]
        role: String,
    },
    
    /// Create ghost identity
    CreateGhost {
        /// Organization ID
        org_id: String,
        
        /// Ghost identity name
        name: String,
        
        /// Purpose description
        #[arg(long)]
        purpose: String,
        
        /// Expiration date (ISO 8601)
        #[arg(long)]
        expires_at: Option<String>,
    },
    
    /// List identities in organization
    ListOrgIdentities {
        /// Organization ID
        org_id: String,
        
        /// Include ghost identities
        #[arg(long)]
        include_ghosts: bool,
    },
    
    /// Grant cross-tenant access
    GrantCrossTenantAccess {
        /// User LogLine ID
        user_id: String,
        
        /// Target tenant ID
        target_tenant: String,
        
        /// Role in target tenant
        #[arg(long, default_value = "viewer")]
        role: String,
        
        /// Expiration date (ISO 8601)
        #[arg(long)]
        expires_at: Option<String>,
    },
    
    /// Revoke cross-tenant access
    RevokeCrossTenantAccess {
        /// User LogLine ID
        user_id: String,
        
        /// Target tenant ID
        target_tenant: String,
    },
}

/// Federation commands
#[derive(Subcommand, Debug)]
pub enum FederationCommand {
    /// Register federated node
    RegisterNode {
        /// Node name
        name: String,
        
        /// Node URL
        url: String,
        
        /// Node public key
        #[arg(long)]
        public_key: String,
        
        /// Trust level (trusted, verified, unverified)
        #[arg(long, default_value = "unverified")]
        trust_level: String,
    },
    
    /// List federated nodes
    ListNodes,
    
    /// Update node trust level
    UpdateTrust {
        /// Node ID
        node_id: String,
        
        /// New trust level
        trust_level: String,
    },
    
    /// Federate with another organization
    Federate {
        /// Target organization domain
        target_org: String,
        
        /// Federation type (full, limited, read_only)
        #[arg(long, default_value = "limited")]
        federation_type: String,
        
        /// Shared namespaces
        #[arg(long)]
        namespaces: Option<Vec<String>>,
    },
    
    /// List federation partnerships
    ListFederations,
    
    /// Sync with federated organizations
    Sync {
        /// Target organization (optional, syncs all if not specified)
        target_org: Option<String>,
        
        /// Dry run (don't actually sync)
        #[arg(long)]
        dry_run: bool,
    },
}

/// Organization configuration for CLI operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationConfig {
    pub id: Uuid,
    pub name: String,
    pub domain: Option<String>,
    pub subdomain: Option<String>,
    pub org_type: String,
    pub branding: OrganizationBranding,
    pub settings: OrganizationSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Organization branding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationBranding {
    pub logo_url: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub custom_css: Option<String>,
    pub favicon_url: Option<String>,
}

/// Organization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSettings {
    pub require_email_verification: bool,
    pub allow_public_registration: bool,
    pub enforce_2fa: bool,
    pub session_timeout_hours: u32,
    pub data_retention_days: Option<u32>,
    pub audit_retention_days: u32,
    pub enable_federation: bool,
    pub allow_ghost_identities: bool,
}

/// CLI tenant context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: Option<String>,
    pub organization_id: Option<Uuid>,
    pub organization_name: Option<String>,
    pub user_role: Option<String>,
    pub permissions: Vec<String>,
    pub switched_at: DateTime<Utc>,
}

impl Default for OrganizationBranding {
    fn default() -> Self {
        OrganizationBranding {
            logo_url: None,
            primary_color: Some("#007AFF".to_string()), // iOS blue
            secondary_color: Some("#5AC8FA".to_string()), // iOS light blue
            custom_css: None,
            favicon_url: None,
        }
    }
}

impl Default for OrganizationSettings {
    fn default() -> Self {
        OrganizationSettings {
            require_email_verification: true,
            allow_public_registration: false,
            enforce_2fa: false,
            session_timeout_hours: 24,
            data_retention_days: None, // Indefinite
            audit_retention_days: 2555, // 7 years
            enable_federation: false,
            allow_ghost_identities: false,
        }
    }
}

/// CLI configuration for multi-tenant operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTenantCliConfig {
    pub current_context: Option<TenantContext>,
    pub organizations: HashMap<String, OrganizationConfig>,
    pub federated_nodes: HashMap<String, FederatedNodeConfig>,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Federated node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedNodeConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub public_key: String,
    pub trust_level: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub sync_status: String,
}

impl MultiTenantCliConfig {
    /// Load configuration from file
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config_path = dirs::home_dir().ok_or("Home directory not found")?;
        config_path.push(".logline");
        config_path.push("multi_tenant_cli.json");
        
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: MultiTenantCliConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(MultiTenantCliConfig::default())
        }
    }
    
    /// Save configuration to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_path = dirs::home_dir().ok_or("Home directory not found")?;
        config_path.push(".logline");
        std::fs::create_dir_all(&config_path)?;
        config_path.push("multi_tenant_cli.json");
        
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        
        Ok(())
    }
    
    /// Switch tenant context
    pub fn switch_context(&mut self, tenant_id: String, organization_id: Option<Uuid>) {
        let org_name = organization_id
            .and_then(|id| {
                self.organizations.values()
                    .find(|org| org.id == id)
                    .map(|org| org.name.clone())
            });
        
        self.current_context = Some(TenantContext {
            tenant_id: Some(tenant_id),
            organization_id,
            organization_name: org_name,
            user_role: None, // Would be populated from roles manager
            permissions: vec![], // Would be populated from roles manager
            switched_at: Utc::now(),
        });
    }
    
    /// Get current tenant ID
    pub fn current_tenant_id(&self) -> Option<&str> {
        self.current_context.as_ref()
            .and_then(|ctx| ctx.tenant_id.as_deref())
    }
    
    /// Add organization to config
    pub fn add_organization(&mut self, org: OrganizationConfig) {
        self.organizations.insert(org.id.to_string(), org);
    }
    
    /// Add federated node to config
    pub fn add_federated_node(&mut self, node: FederatedNodeConfig) {
        self.federated_nodes.insert(node.id.clone(), node);
    }
}

impl Default for MultiTenantCliConfig {
    fn default() -> Self {
        MultiTenantCliConfig {
            current_context: None,
            organizations: HashMap::new(),
            federated_nodes: HashMap::new(),
            last_sync: None,
        }
    }
}