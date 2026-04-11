//! POST /v1/chat/completions endpoint

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Sse},
    Json,
};
use axum::response::sse::Event;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{pin::Pin, sync::Arc};
use tokio::sync::mpsc;
use tracing::{error, info};

use nanobot_bus::{InboundMessage, MessageBus};
use nanobot_config::Config;

/// Chat completion request (OpenAI-compatible)
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub top_p: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub n: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub stop: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub frequency_penalty: Option<f32>,
}

fn default_max_tokens() -> Option<u32> {
    Some(8192)
}

/// Chat message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: ChatContent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Parts(Vec<serde_json::Value>),
}

impl ChatContent {
    pub fn as_text(&self) -> String {
        match self {
            ChatContent::Text(s) => s.clone(),
            ChatContent::Parts(parts) => {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
    }
}

/// Chat completion response
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Stream delta
#[derive(Debug, Clone, Serialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
}

/// Stream chunk
#[derive(Debug, Clone, Serialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub choices: Vec<StreamChoice>,
}

/// Shared state for the API
pub struct ApiState {
    pub config: Config,
    pub message_bus: MessageBus,
}

/// Handler for POST /v1/chat/completions
pub async fn create_chat_completion(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    info!("Chat completion request: model={}, stream={}, messages={}",
          request.model, request.stream, request.messages.len());

    if request.stream {
        // Streaming response
        stream_chat_completion(state, request).await.into_response()
    } else {
        // Non-streaming response
        match complete_chat(state, request).await {
            Ok(response) => Json(response).into_response(),
            Err(e) => {
                error!("Chat completion error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": {
                            "message": e.to_string(),
                            "type": "internal_error",
                        }
                    }))
                ).into_response()
            }
        }
    }
}

async fn complete_chat(
    state: Arc<ApiState>,
    request: ChatCompletionRequest,
) -> Result<ChatCompletionResponse, anyhow::Error> {
    // Get the last user message
    let user_message = request.messages
        .iter()
        .filter(|m| m.role == "user")
        .last()
        .ok_or_else(|| anyhow::anyhow!("No user message found"))?;

    // Create inbound message
    let inbound = InboundMessage::new(
        "api",
        "api_user",
        "api_session",
        user_message.content.as_text(),
    );

    // Publish to message bus
    state.message_bus.publish_inbound(inbound).await?;

    // Wait for response (simplified - in production would use a proper request/response pattern)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Try to get outbound response
    let response_text = match tokio::time::timeout(
        tokio::time::Duration::from_secs(30),
        state.message_bus.consume_outbound()
    ).await {
        Ok(Ok(outbound)) => outbound.content,
        Ok(Err(_)) => "Error: No response from agent".to_string(),
        Err(_) => "Error: Request timeout".to_string(),
    };

    Ok(ChatCompletionResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp() as u64,
        model: request.model,
        system_fingerprint: Some("rustbot".to_string()),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(response_text),
            },
            logprobs: None,
            finish_reason: "stop".to_string(),
        }],
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 10,
            total_tokens: 20,
        }),
    })
}

async fn stream_chat_completion(
    state: Arc<ApiState>,
    request: ChatCompletionRequest,
) -> Sse<impl Stream<Item = Result<Event, anyhow::Error>>> {
    let (tx, rx) = mpsc::channel(100);

    // Get the last user message
    let user_message = request.messages
        .iter()
        .filter(|m| m.role == "user")
        .last()
        .cloned();

    if let Some(msg) = user_message {
        let inbound = InboundMessage::new(
            "api",
            "api_user",
            "api_session",
            msg.content.as_text(),
        );

        let bus_clone = state.message_bus.clone();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            // Publish message
            if let Err(e) = bus_clone.publish_inbound(inbound).await {
                error!("Failed to publish message: {}", e);
                let _ = tx_clone.send(Err(anyhow::anyhow!("Failed to publish message: {}", e))).await;
                return;
            }

            // Stream responses
            let mut count = 0;
            while let Ok(Ok(outbound)) = tokio::time::timeout(
                tokio::time::Duration::from_secs(60),
                bus_clone.consume_outbound()
            ).await {
                let content = outbound.content;

                let event = Event::default()
                    .json_data(StreamChunk {
                        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                        object: "chat.completion.chunk".to_string(),
                        created: chrono::Utc::now().timestamp() as u64,
                        model: request.model.clone(),
                        system_fingerprint: Some("rustbot".to_string()),
                        choices: vec![StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: if count == 0 { Some("assistant".to_string()) } else { None },
                                content: Some(content),
                            },
                            logprobs: None,
                            finish_reason: None,
                        }],
                    });

                match event {
                    Ok(event) => {
                        if tx_clone.send(Ok(event)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize chunk: {}", e);
                        let _ = tx_clone.send(Err(anyhow::anyhow!("Serialization error: {}", e))).await;
                        break;
                    }
                }

                count += 1;
            }

            // Send end marker
            let _ = tx_clone.send(Ok(Event::default().data("[DONE]"))).await;
        });
    }

    let stream: Pin<Box<dyn Stream<Item = Result<Event, anyhow::Error>> + Send>> = Box::pin(
        tokio_stream::wrappers::ReceiverStream::new(rx)
    );

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(tokio::time::Duration::from_secs(15))
    )
}
