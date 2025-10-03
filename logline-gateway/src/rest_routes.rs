use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{OriginalUri, State};
use axum::http::{self, HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use http_body_util::BodyExt;
use tracing::{debug, instrument, warn};

use crate::discovery::ServiceDiscovery;

#[derive(Clone)]
pub struct RestProxyState {
    client: reqwest::Client,
    targets: HashMap<String, String>,
}

impl RestProxyState {
    pub fn new(client: reqwest::Client, discovery: &ServiceDiscovery) -> Self {
        Self {
            client,
            targets: discovery.rest_targets(),
        }
    }

    fn resolve(&self, uri: &Uri) -> Result<(String, String), StatusCode> {
        let path = uri.path();
        let mut segments = path.trim_start_matches('/').splitn(2, '/');
        let service_key = segments.next().unwrap_or("");

        if service_key.is_empty() {
            return Err(StatusCode::NOT_FOUND);
        }

        let target_base = self
            .targets
            .get(service_key)
            .ok_or(StatusCode::NOT_FOUND)?
            .clone();
        let remainder = segments.next().unwrap_or("");
        let mut forward_url = target_base.trim_end_matches('/').to_string();
        if !remainder.is_empty() {
            if !forward_url.ends_with('/') {
                forward_url.push('/');
            }
            forward_url.push_str(remainder);
        }

        if let Some(query) = uri.query() {
            forward_url.push('?');
            forward_url.push_str(query);
        }

        Ok((service_key.to_string(), forward_url))
    }
}

pub fn router(state: RestProxyState) -> Router {
    Router::new()
        .route("/engine", any(proxy_request))
        .route("/engine/*rest", any(proxy_request))
        .route("/rules", any(proxy_request))
        .route("/rules/*rest", any(proxy_request))
        .route("/timeline", any(proxy_request))
        .route("/timeline/*rest", any(proxy_request))
        .route("/id", any(proxy_request))
        .route("/id/*rest", any(proxy_request))
        .route("/federation", any(proxy_request))
        .route("/federation/*rest", any(proxy_request))
        .with_state(state)
}

#[instrument(skip_all, fields(method = tracing::field::Empty, service = tracing::field::Empty))]
async fn proxy_request(
    State(state): State<RestProxyState>,
    method: Method,
    headers: HeaderMap,
    OriginalUri(original_uri): OriginalUri,
    body: Body,
) -> Result<Response, StatusCode> {
    let (service_key, target_url) = state.resolve(&original_uri)?;
    tracing::Span::current().record("method", &tracing::field::display(&method));
    tracing::Span::current().record("service", &tracing::field::display(&service_key));

    let req_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut request_builder = state.client.request(req_method, &target_url);

    for (name, value) in headers.iter() {
        if name == http::header::HOST || name == http::header::CONTENT_LENGTH {
            continue;
        }

        let header_name = reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes())
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let header_value = reqwest::header::HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        request_builder = request_builder.header(header_name, header_value);
    }

    let body_bytes = body
        .collect()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .to_bytes();
    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.clone());
    }

    debug!(%target_url, "forwarding REST request");

    let response = request_builder.send().await.map_err(|err| {
        warn!(%service_key, ?err, "falha ao encaminhar requisição REST");
        StatusCode::BAD_GATEWAY
    })?;

    let status =
        StatusCode::from_u16(response.status().as_u16()).map_err(|_| StatusCode::BAD_GATEWAY)?;
    let mut builder = Response::builder().status(status);

    for (name, value) in response.headers() {
        let name_str = name.as_str();
        if name_str.eq_ignore_ascii_case(http::header::CONTENT_LENGTH.as_str())
            || name_str.eq_ignore_ascii_case(http::header::TRANSFER_ENCODING.as_str())
        {
            continue;
        }

        if let (Ok(header_name), Ok(header_value)) = (
            http::header::HeaderName::from_bytes(name_str.as_bytes()),
            http::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            builder = builder.header(header_name, header_value);
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    builder
        .body(Body::from(bytes))
        .map_err(|_| StatusCode::BAD_GATEWAY)
}
