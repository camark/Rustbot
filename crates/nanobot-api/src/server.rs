//! API Server implementation

use axum::{
    routing::{get, post},
    Router,
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::auth::ApiAuth;
use crate::routes::{list_models, get_model, create_chat_completion, ApiState};

/// API Server configuration
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    pub host: String,
    pub port: u16,
    pub api_key: Option<String>,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8900,
            api_key: None,
        }
    }
}

/// API Server
pub struct ApiServer {
    config: ApiServerConfig,
    auth: Arc<ApiAuth>,
    state: Arc<ApiState>,
}

impl ApiServer {
    /// Get server host
    pub fn host(&self) -> &str {
        &self.config.host
    }

    /// Get server port
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Check if auth is enabled
    pub fn is_auth_enabled(&self) -> Arc<ApiAuth> {
        self.auth.clone()
    }

    /// Get a clone of the auth instance
    pub fn auth(&self) -> Arc<ApiAuth> {
        self.auth.clone()
    }
    /// Create a new API server
    pub fn new(config: ApiServerConfig, state: ApiState) -> Self {
        let auth = if let Some(key) = &config.api_key {
            Arc::new(ApiAuth::with_key(key))
        } else {
            Arc::new(ApiAuth::new())
        };

        Self {
            config,
            auth,
            state: Arc::new(state),
        }
    }

    /// Create with API key authentication
    pub fn with_auth(config: ApiServerConfig, api_key: String, state: ApiState) -> Self {
        Self {
            config,
            auth: Arc::new(ApiAuth::with_key(api_key)),
            state: Arc::new(state),
        }
    }

    /// Build the router
    fn build_router(&self) -> Router {
        let auth_middleware = middleware::from_fn_with_state(
            self.auth.clone(),
            auth_middleware,
        );

        Router::new()
            // Health check
            .route("/health", get(health_check))
            // OpenAI-compatible API
            .route("/v1/models", get(list_models))
            .route("/v1/models/:id", get(get_model))
            .route("/v1/chat/completions", post(create_chat_completion))
            // Apply auth middleware to API routes
            .layer(auth_middleware)
            .with_state(self.state.clone())
    }

    /// Start the server
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let listener = TcpListener::bind(addr).await?;

        info!("Starting API server on http://{}", addr);
        if self.auth.is_enabled().await {
            info!("API key authentication enabled");
        } else {
            warn!("API key authentication disabled - anyone can access the API");
        }

        let app = self.build_router();

        axum::serve(listener, app).await?;
        Ok(())
    }

    /// Start the server with a custom listener
    pub async fn run_with_listener(&self, listener: TcpListener) -> Result<(), anyhow::Error> {
        info!("Starting API server");
        if self.auth.is_enabled().await {
            info!("API key authentication enabled");
        } else {
            warn!("API key authentication disabled");
        }

        let app = self.build_router();
        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// Auth middleware
async fn auth_middleware(
    State(auth): State<Arc<ApiAuth>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    // Check if auth is enabled
    if !auth.is_enabled().await {
        return Ok(next.run(req).await);
    }

    // Extract and validate API key
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    if let Some(key) = crate::auth::extract_api_key(auth_header) {
        if auth.is_valid(&key).await {
            return Ok(next.run(req).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

/// Health check endpoint
async fn health_check() -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Start the API server
pub async fn start_api_server(
    host: &str,
    port: u16,
    api_key: Option<&str>,
    state: ApiState,
) -> Result<(), anyhow::Error> {
    let config = ApiServerConfig {
        host: host.to_string(),
        port,
        api_key: api_key.map(String::from),
    };

    let server = ApiServer::new(config, state);
    server.run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config() {
        let config = ApiServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8900);
    }
}
