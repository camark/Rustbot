//! GET /v1/models endpoint

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

/// List models response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

/// Single model info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

impl Default for ModelsResponse {
    fn default() -> Self {
        Self {
            object: "list".to_string(),
            data: vec![
                ModelInfo {
                    id: "rustbot/default".to_string(),
                    object: "model".to_string(),
                    created: chrono::Utc::now().timestamp() as u64,
                    owned_by: "rustbot".to_string(),
                },
            ],
        }
    }
}

/// Handler for GET /v1/models
pub async fn list_models() -> impl IntoResponse {
    let response = ModelsResponse::default();
    Json(response)
}

/// Handler for GET /v1/models/:id
pub async fn get_model(axum::extract::Path(model_id): axum::extract::Path<String>) -> impl IntoResponse {
    let models = ModelsResponse::default();

    for model in models.data {
        if model.id == model_id {
            return Json(model).into_response();
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": {
                "message": format!("Model '{}' not found", model_id),
                "type": "invalid_request_error",
                "code": "model_not_found",
            }
        }))
    ).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_models_response() {
        let response = ModelsResponse::default();
        assert_eq!(response.object, "list");
        assert!(!response.data.is_empty());
    }
}
