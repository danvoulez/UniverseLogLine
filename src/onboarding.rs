use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use colored::*;
use dirs::home_dir;
use logline_core::identity::LogLineID;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum OnboardingCliError {
    #[error("requisição HTTP falhou: {0}")]
    Http(String),
    #[error("gateway retornou erro: {0}")]
    Gateway(String),
    #[error("persistência da sessão falhou: {0}")]
    Storage(String),
    #[error("nenhuma sessão ativa; crie uma identidade primeiro")]
    NoActiveSession,
    #[error("sessão para handle '{0}' não encontrada. execute 'logline create identity' primeiro")]
    UnknownSession(String),
    #[error("{0}")]
    Validation(String),
}

impl From<reqwest::Error> for OnboardingCliError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value.to_string())
    }
}

impl From<io::Error> for OnboardingCliError {
    fn from(value: io::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionData {
    sessions: HashMap<String, StoredSession>,
    active_handle: Option<String>,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            active_handle: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub handle: String,
    pub session_id: Uuid,
    pub logline_id: LogLineID,
    pub signing_key: String,
    pub tenant_id: Option<String>,
    pub jwt: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl StoredSession {
    pub fn new(handle: &str, session_id: Uuid, logline_id: LogLineID, signing_key: String) -> Self {
        Self {
            handle: handle.to_string(),
            session_id,
            logline_id,
            signing_key,
            tenant_id: None,
            jwt: None,
            updated_at: Utc::now(),
        }
    }
}

pub struct SessionStore {
    path: PathBuf,
    data: SessionData,
}

impl SessionStore {
    pub fn load() -> Result<Self, OnboardingCliError> {
        let path = session_file_path()?;
        let data = if path.exists() {
            let contents = fs::read_to_string(&path)?;
            serde_json::from_str(&contents).map_err(|err| {
                OnboardingCliError::Storage(format!("arquivo de sessão inválido: {err}"))
            })?
        } else {
            SessionData::default()
        };

        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<(), OnboardingCliError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_string_pretty(&self.data)
            .map_err(|err| OnboardingCliError::Storage(err.to_string()))?;
        fs::write(&self.path, payload)?;
        Ok(())
    }

    pub fn upsert(&mut self, session: StoredSession) {
        self.data.active_handle = Some(session.handle.clone());
        self.data.sessions.insert(session.handle.clone(), session);
    }

    pub fn set_active(&mut self, handle: &str) -> Result<(), OnboardingCliError> {
        if !self.data.sessions.contains_key(handle) {
            return Err(OnboardingCliError::UnknownSession(handle.to_string()));
        }
        self.data.active_handle = Some(handle.to_string());
        Ok(())
    }

    pub fn active_session_id(&self) -> Result<(String, Uuid), OnboardingCliError> {
        match &self.data.active_handle {
            Some(handle) => {
                let session = self
                    .data
                    .sessions
                    .get(handle)
                    .ok_or_else(|| OnboardingCliError::UnknownSession(handle.clone()))?;
                Ok((handle.clone(), session.session_id))
            }
            None => Err(OnboardingCliError::NoActiveSession),
        }
    }

    pub fn session(&self, handle: &str) -> Result<&StoredSession, OnboardingCliError> {
        self.data
            .sessions
            .get(handle)
            .ok_or_else(|| OnboardingCliError::UnknownSession(handle.to_string()))
    }

    pub fn session_mut(&mut self, handle: &str) -> Result<&mut StoredSession, OnboardingCliError> {
        self.data
            .sessions
            .get_mut(handle)
            .ok_or_else(|| OnboardingCliError::UnknownSession(handle.to_string()))
    }
}

fn session_file_path() -> Result<PathBuf, OnboardingCliError> {
    let mut path = home_dir().ok_or_else(|| {
        OnboardingCliError::Storage("não foi possível determinar diretório home".into())
    })?;
    path.push(".logline");
    path.push("sessions");
    fs::create_dir_all(&path)?;
    path.push("onboarding.json");
    Ok(path)
}

pub struct OnboardingClient {
    base_url: Url,
    http: reqwest::Client,
}

impl OnboardingClient {
    pub fn new(base_url: &str) -> Result<Self, OnboardingCliError> {
        let url = Url::parse(base_url).map_err(|err| {
            OnboardingCliError::Validation(format!("URL do gateway inválida: {err}"))
        })?;
        Ok(Self {
            base_url: url,
            http: reqwest::Client::new(),
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, OnboardingCliError> {
        self.base_url
            .join(path)
            .map_err(|err| OnboardingCliError::Validation(format!("caminho inválido: {err}")))
    }

    pub async fn create_identity(
        &self,
        request: CreateIdentityRequest,
    ) -> Result<CreateIdentityResponse, OnboardingCliError> {
        let url = self.endpoint("/onboarding/identity")?;
        let response = self.http.post(url).json(&request).send().await?;
        parse_response(response).await
    }

    pub async fn create_tenant(
        &self,
        request: CreateTenantRequest,
    ) -> Result<CreateTenantResponse, OnboardingCliError> {
        let url = self.endpoint("/onboarding/tenant")?;
        let response = self.http.post(url).json(&request).send().await?;
        parse_response(response).await
    }

    pub async fn assign_identity(
        &self,
        request: AssignIdentityRequest,
    ) -> Result<AssignIdentityResponse, OnboardingCliError> {
        let url = self.endpoint("/onboarding/assignment")?;
        let response = self.http.post(url).json(&request).send().await?;
        parse_response(response).await
    }

    pub async fn select_template(
        &self,
        request: SelectTemplateRequest,
    ) -> Result<SelectTemplateResponse, OnboardingCliError> {
        let url = self.endpoint("/onboarding/template")?;
        let response = self.http.post(url).json(&request).send().await?;
        parse_response(response).await
    }

    pub async fn declare_purpose(
        &self,
        request: DeclarePurposeRequest,
    ) -> Result<DeclarePurposeResponse, OnboardingCliError> {
        let url = self.endpoint("/onboarding/purpose")?;
        let response = self.http.post(url).json(&request).send().await?;
        parse_response(response).await
    }

    pub async fn execute_shell(
        &self,
        request: ExecuteShellRequest,
    ) -> Result<ExecuteShellResponse, OnboardingCliError> {
        let url = self.endpoint("/onboarding/run")?;
        let response = self.http.post(url).json(&request).send().await?;
        parse_response(response).await
    }
}

async fn parse_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, OnboardingCliError> {
    if response.status().is_success() {
        response
            .json::<T>()
            .await
            .map_err(|err| OnboardingCliError::Http(err.to_string()))
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        if let Ok(err) = serde_json::from_str::<GatewayError>(&body) {
            Err(OnboardingCliError::Gateway(format!(
                "{status}: {}",
                err.error
            )))
        } else {
            Err(OnboardingCliError::Gateway(format!("{status}: {body}")))
        }
    }
}

#[derive(Debug, Deserialize)]
struct GatewayError {
    error: String,
}

#[derive(Debug, Serialize)]
pub struct CreateIdentityRequest {
    pub name: String,
    pub handle: String,
    pub ghost: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateIdentityResponse {
    pub session_id: Uuid,
    pub handle: String,
    pub identity: IdentityPayload,
    pub timeline_entry_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct IdentityPayload {
    pub name: String,
    pub ghost: bool,
    pub logline_id: LogLineID,
    pub signing_key: String,
}

#[derive(Debug, Serialize)]
pub struct CreateTenantRequest {
    pub session_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantResponse {
    pub session_id: Uuid,
    pub tenant_id: String,
    pub timeline_entry_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct AssignIdentityRequest {
    pub session_id: Uuid,
    pub handle: String,
    pub tenant_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignIdentityResponse {
    pub session_id: Uuid,
    pub tenant_id: String,
    pub jwt: String,
    pub timeline_entry_id: Uuid,
    pub signing_key: String,
}

#[derive(Debug, Serialize)]
pub struct SelectTemplateRequest {
    pub session_id: Uuid,
    pub template: String,
    pub owner: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SelectTemplateResponse {
    pub session_id: Uuid,
    pub template: String,
    pub timeline_entry_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct DeclarePurposeRequest {
    pub session_id: Uuid,
    pub app: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct DeclarePurposeResponse {
    pub session_id: Uuid,
    pub timeline_entry_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ExecuteShellRequest {
    pub session_id: Uuid,
    pub command: String,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteShellResponse {
    pub session_id: Uuid,
    pub executed_at: DateTime<Utc>,
    pub timeline_entry_id: Uuid,
}

pub async fn read_shell_command(prompt: &str) -> Result<String, OnboardingCliError> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut line = String::new();
    let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
    reader.read_line(&mut line).await?;
    Ok(line.trim().to_string())
}

pub fn slugify(value: &str) -> String {
    let mut result = String::new();
    let mut previous_hyphen = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            previous_hyphen = false;
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !previous_hyphen && !result.is_empty() {
                result.push('-');
                previous_hyphen = true;
            }
        }
    }

    if result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "tenant".to_string()
    } else {
        result
    }
}

pub fn print_identity_created(response: &CreateIdentityResponse) {
    println!(
        "{} {}",
        "✔ Identidade computável criada:".green().bold(),
        response.identity.name.bold()
    );
    println!("  Handle: {}", response.handle);
    if response.identity.ghost {
        println!("  Modo: ghost");
    }
    println!("  LogLine ID: {}", response.identity.logline_id.id);
    println!("  Sessão: {}", response.session_id);
    println!("  Timeline span: {}", response.timeline_entry_id);
}

pub fn print_tenant_created(response: &CreateTenantResponse, tenant_name: &str) {
    println!(
        "{} {} (id: {})",
        "✔ Tenant criado:".green().bold(),
        tenant_name.bold(),
        response.tenant_id
    );
    println!("  Sessão: {}", response.session_id);
    println!("  Timeline span: {}", response.timeline_entry_id);
}

pub fn print_assignment(response: &AssignIdentityResponse, handle: &str) {
    println!(
        "{} {} → {}",
        "✔ Identidade atribuída:".green().bold(),
        handle.bold(),
        response.tenant_id
    );
    println!("  Sessão: {}", response.session_id);
    println!("  JWT emitido: {}", response.jwt);
    println!("  Timeline span: {}", response.timeline_entry_id);
}

pub fn print_template_selected(response: &SelectTemplateResponse) {
    println!(
        "{} {}",
        "✔ Template inicializado:".green().bold(),
        response.template.bold()
    );
    println!("  Sessão: {}", response.session_id);
    println!("  Timeline span: {}", response.timeline_entry_id);
}

pub fn print_purpose(response: &DeclarePurposeResponse) {
    println!("{}", "✔ Propósito computável registrado".green().bold());
    println!("  Sessão: {}", response.session_id);
    println!("  Timeline span: {}", response.timeline_entry_id);
}

pub fn print_shell_execution(response: &ExecuteShellResponse, command: &str) {
    println!(
        "{} {}",
        "✔ Execução computável registrada:".green().bold(),
        command
    );
    println!("  Sessão: {}", response.session_id);
    println!("  Timestamp: {}", response.executed_at);
    println!("  Timeline span: {}", response.timeline_entry_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_matches_gateway() {
        assert_eq!(slugify("VoulezVous"), "voulezvous");
        assert_eq!(slugify("Voulez Vous"), "voulez-vous");
        assert_eq!(slugify("  ACME Labs  "), "acme-labs");
    }
}
