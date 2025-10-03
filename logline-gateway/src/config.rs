use std::env;

use logline_core::errors::ConfigError;
use tracing::warn;
use url::Url;

/// REST e WebSocket URLs para um microserviço.
#[derive(Debug, Clone)]
pub struct ServiceUrls {
    pub key: &'static str,
    pub service_name: &'static str,
    pub rest_url: String,
    pub ws_url: Option<String>,
    pub health_path: &'static str,
}

impl ServiceUrls {
    pub fn new(
        key: &'static str,
        service_name: &'static str,
        rest_url: String,
        ws_url: Option<String>,
    ) -> Self {
        Self {
            key,
            service_name,
            rest_url,
            ws_url,
            health_path: "/health",
        }
    }
}

/// Configuração global do gateway carregada a partir das variáveis de ambiente.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub bind_address: String,
    pub engine: ServiceUrls,
    pub rules: ServiceUrls,
    pub timeline: ServiceUrls,
    pub identity: ServiceUrls,
    pub federation: ServiceUrls,
}

impl GatewayConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_address = env::var("GATEWAY_BIND").unwrap_or_else(|_| "0.0.0.0:8070".to_string());

        let engine_rest = read_http_url("ENGINE_URL", "http://127.0.0.1:8090")?;
        let engine_ws = read_ws_url("ENGINE_WS_URL")?.or_else(|| derive_ws_url(&engine_rest).ok());

        let rules_rest = read_http_url("RULES_URL", "http://127.0.0.1:8081")?;
        let rules_ws = read_ws_url("RULES_WS_URL")?.or_else(|| derive_ws_url(&rules_rest).ok());

        let timeline_rest = read_http_url("TIMELINE_URL", "http://127.0.0.1:8082")?;
        let timeline_ws =
            read_ws_url("TIMELINE_WS_URL")?.or_else(|| derive_ws_url(&timeline_rest).ok());

        let identity_rest = read_http_url("ID_URL", "http://127.0.0.1:8083")?;
        let identity_ws = read_ws_url("ID_WS_URL")?.or_else(|| derive_ws_url(&identity_rest).ok());

        let federation_rest = read_http_url("FEDERATION_URL", "http://127.0.0.1:8084")?;
        let federation_ws =
            read_ws_url("FEDERATION_WS_URL")?.or_else(|| derive_ws_url(&federation_rest).ok());

        Ok(Self {
            bind_address,
            engine: ServiceUrls::new("engine", "logline-engine", engine_rest, engine_ws),
            rules: ServiceUrls::new("rules", "logline-rules", rules_rest, rules_ws),
            timeline: ServiceUrls::new("timeline", "logline-timeline", timeline_rest, timeline_ws),
            identity: ServiceUrls::new("id", "logline-id", identity_rest, identity_ws),
            federation: ServiceUrls::new(
                "federation",
                "logline-federation",
                federation_rest,
                federation_ws,
            ),
        })
    }

    pub fn services(&self) -> Vec<ServiceUrls> {
        vec![
            self.engine.clone(),
            self.rules.clone(),
            self.timeline.clone(),
            self.identity.clone(),
            self.federation.clone(),
        ]
    }

    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }
}

fn read_http_url(key: &'static str, default: &str) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(value) => sanitize_http_url(key, value.trim()),
        Err(env::VarError::NotPresent) => sanitize_http_url(key, default),
        Err(err) => Err(ConfigError::InvalidEnvVar { key, source: err }),
    }
}

fn read_ws_url(key: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(key) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                sanitize_ws_url(key, trimmed).map(Some)
            }
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(ConfigError::InvalidEnvVar { key, source: err }),
    }
}

fn sanitize_http_url(key: &'static str, value: &str) -> Result<String, ConfigError> {
    let parsed = Url::parse(value)
        .map_err(|err| ConfigError::Internal(format!("URL inválida para {key}: {err}")))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(ConfigError::Internal(format!(
            "URL de {key} precisa usar http ou https"
        )));
    }

    Ok(trim_trailing_slash(value))
}

fn sanitize_ws_url(key: &'static str, value: &str) -> Result<String, ConfigError> {
    let parsed = Url::parse(value)
        .map_err(|err| ConfigError::Internal(format!("URL inválida para {key}: {err}")))?;

    if parsed.scheme() != "ws" && parsed.scheme() != "wss" {
        return Err(ConfigError::Internal(format!(
            "URL de {key} precisa usar ws ou wss"
        )));
    }

    Ok(value.to_string())
}

fn derive_ws_url(http_url: &str) -> Result<String, ConfigError> {
    let mut parsed = Url::parse(http_url)
        .map_err(|err| ConfigError::Internal(format!("URL inválida: {err}")))?;
    let scheme = match parsed.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => {
            warn!("esquema {other} não suportado para conversão em WebSocket");
            return Err(ConfigError::Internal(format!(
                "não foi possível derivar URL WebSocket a partir de {http_url}"
            )));
        }
    };

    parsed
        .set_scheme(scheme)
        .map_err(|_| ConfigError::Internal("falha ao definir esquema de WebSocket".into()))?;

    let mut path = parsed.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        path.push('/');
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str("ws/service");
    parsed.set_path(&path);
    parsed.set_query(None);
    parsed.set_fragment(None);

    Ok(parsed.to_string())
}

fn trim_trailing_slash(value: &str) -> String {
    if value.ends_with('/') {
        value.trim_end_matches('/').to_string()
    } else {
        value.to_string()
    }
}
