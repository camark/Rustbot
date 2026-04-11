//! Base LLM provider trait and types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};
use serde_json::Value;
use std::pin::Pin;
use std::future::Future;

use crate::error::Result;

// ============================================================================
// Core Types
// ============================================================================

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: u32,

    #[serde(default)]
    pub completion_tokens: u32,

    #[serde(default)]
    pub total_tokens: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
}

/// A tool call request from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_content: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_specific_fields: Option<Value>,
}

impl ToolCall {
    /// Convert to OpenAI-style tool call payload
    pub fn to_openai_tool_call(&self) -> Value {
        let mut tool_call = serde_json::json!({
            "id": self.id,
            "type": "function",
            "function": {
                "name": self.name,
                "arguments": self.arguments,
            },
        });

        if let Some(extra) = &self.extra_content {
            tool_call["extra_content"] = extra.clone();
        }
        if let Some(fields) = &self.provider_specific_fields {
            tool_call["provider_specific_fields"] = fields.clone();
        }

        tool_call
    }
}

/// Tool definition schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,

    pub function: FunctionDefinition,
}

impl ToolDefinition {
    pub fn new(name: String, description: String, parameters: Value) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name,
                description,
                parameters,
            },
        }
    }
}

/// Function definition for tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Response from an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,

    #[serde(default = "default_finish_reason")]
    pub finish_reason: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_blocks: Option<Value>,
}

fn default_finish_reason() -> String {
    "stop".to_string()
}

impl LLMResponse {
    /// Check if response has tool calls
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// Check if response is an error
    pub fn is_error(&self) -> bool {
        self.finish_reason == "error"
    }
}

impl Default for LLMResponse {
    fn default() -> Self {
        Self {
            content: None,
            tool_calls: Vec::new(),
            finish_reason: "stop".to_string(),
            usage: None,
            reasoning_content: None,
            thinking_blocks: None,
        }
    }
}

// ============================================================================
// Generation Settings
// ============================================================================

/// Default generation parameters for LLM calls
#[derive(Debug, Clone)]
pub struct GenerationSettings {
    pub temperature: f32,
    pub max_tokens: u32,
    pub reasoning_effort: Option<String>,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            reasoning_effort: None,
        }
    }
}

// ============================================================================
// Chat Request
// ============================================================================

/// Chat completion request
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub model: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub reasoning_effort: Option<String>,
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            model: None,
            tools: None,
            max_tokens: None,
            temperature: None,
            reasoning_effort: None,
            tool_choice: None,
            stream: false,
        }
    }
}

/// Message role and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_tool_calls_opt")]
    pub tool_calls: Option<Vec<ToolCall>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Deserialize Option<Vec<ToolCall>> from OpenAI format (with nested function object) or flat format
fn deserialize_tool_calls_opt<'de, D>(deserializer: D) -> std::result::Result<Option<Vec<ToolCall>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(values) => {
            let tool_calls: std::result::Result<Vec<ToolCall>, _> = values.into_iter()
                .map(|v| {
                    // Try to detect format: OpenAI has "function" nested, flat has direct fields
                    if let Some(obj) = v.as_object() {
                        if obj.contains_key("function") {
                            // OpenAI format: {id, type: "function", function: {name, arguments}}
                            #[derive(Deserialize)]
                            struct OpenAiFormat {
                                id: String,
                                function: OpenAiFunction,
                            }
                            #[derive(Deserialize)]
                            struct OpenAiFunction {
                                name: String,
                                arguments: Value,
                            }
                            let openai: OpenAiFormat = serde_json::from_value(v)
                                .map_err(de::Error::custom)?;
                            Ok(ToolCall {
                                id: openai.id,
                                name: openai.function.name,
                                arguments: openai.function.arguments,
                                extra_content: None,
                                provider_specific_fields: None,
                            })
                        } else {
                            // Flat format: {id, name, arguments}
                            serde_json::from_value(v).map_err(de::Error::custom)
                        }
                    } else {
                        Err(de::Error::custom("tool_call must be an object"))
                    }
                })
                .collect();
            Ok(Some(tool_calls?))
        }
    }
}

impl Message {
    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool(content: impl Into<MessageContent>, tool_call_id: String) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
            name: None,
        }
    }

    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
}

/// Message content (text or multimodal)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

/// Content block for multimodal messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentBlock {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    InputAudio { input_audio: InputAudio },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAudio {
    pub data: String,
    pub format: String,
}

/// Tool choice strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    #[serde(rename = "none")]
    None,

    #[serde(rename = "auto")]
    Auto,

    #[serde(rename = "required")]
    Required,

    Specific {
        #[serde(rename = "type")]
        tool_type: String,
        function: SpecificFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecificFunction {
    pub name: String,
}

// ============================================================================
// Provider Trait
// ============================================================================

/// Callback type for streaming deltas
pub type StreamCallback = Box<dyn FnMut(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

/// LLM Provider trait
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Get the default model for this provider
    fn get_default_model(&self) -> &str;

    /// Get generation settings
    fn get_generation_settings(&self) -> &GenerationSettings;

    /// Send a chat completion request
    async fn chat(&self, request: ChatRequest) -> Result<LLMResponse>;

    /// Send a streaming chat completion request
    async fn chat_stream(
        &self,
        request: ChatRequest,
        on_delta: StreamCallback,
    ) -> Result<LLMResponse> {
        // Default implementation falls back to non-streaming
        let response = self.chat(request).await?;
        if let Some(content) = &response.content {
            let mut callback = on_delta;
            callback(content.clone()).await;
        }
        Ok(response)
    }

    /// Chat with retry on transient errors
    async fn chat_with_retry(&self, request: ChatRequest) -> Result<LLMResponse> {
        let retry_delays = [1, 2, 4];

        for (attempt, delay) in retry_delays.iter().enumerate() {
            let response = self.chat(request.clone()).await?;

            if !response.is_error() {
                return Ok(response);
            }

            // Check if error is transient
            if let Some(content) = &response.content {
                if !is_transient_error(content) {
                    return Ok(response);
                }
            }

            tracing::warn!(
                "LLM transient error (attempt {}/{}, retrying in {}s)",
                attempt + 1,
                retry_delays.len(),
                delay
            );

            tokio::time::sleep(std::time::Duration::from_secs(*delay)).await;
        }

        self.chat(request).await
    }
}

/// Check if error message indicates a transient error
fn is_transient_error(content: &str) -> bool {
    let err = content.to_lowercase();
    let transient_markers = [
        "429", "rate limit", "500", "502", "503", "504",
        "overloaded", "timeout", "timed out", "connection",
        "server error", "temporarily unavailable",
    ];

    transient_markers.iter().any(|marker| err.contains(marker))
}
