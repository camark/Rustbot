//! OpenAI-compatible provider implementation
//!
//! Works with: OpenAI, OpenRouter, DeepSeek, Moonshot, vLLM, Ollama, etc.

use async_trait::async_trait;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, error, warn};

use crate::base::{
    ChatRequest, GenerationSettings, LLMProvider, LLMResponse, Message, MessageContent,
    StreamCallback, ToolCall, ToolChoice, TokenUsage,
};
use crate::error::{ProviderError, Result};

/// OpenAI-compatible provider
pub struct OpenAiCompatProvider {
    client: Client,
    api_key: String,
    api_base: String,
    model: String,
    generation: GenerationSettings,
    strip_model_prefix: bool,
}

impl OpenAiCompatProvider {
    /// Create a new OpenAI-compatible provider
    pub fn new(
        api_key: String,
        api_base: String,
        model: String,
        strip_model_prefix: bool,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            api_base,
            model,
            generation: GenerationSettings::default(),
            strip_model_prefix,
        }
    }

    /// Set generation settings
    pub fn with_generation_settings(mut self, settings: GenerationSettings) -> Self {
        self.generation = settings;
        self
    }

    /// Get the model name (optionally stripped of prefix)
    fn get_model_name(&self) -> String {
        if self.strip_model_prefix {
            // Strip provider prefix like "anthropic/" or "openai/"
            self.model
                .split('/')
                .last()
                .unwrap_or(&self.model)
                .to_string()
        } else {
            self.model.clone()
        }
    }

    /// Build request body
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|msg| self.message_to_openai_format(msg))
            .collect();

        let mut body = json!({
            "model": self.get_model_name(),
            "messages": messages,
            "stream": request.stream,
        });

        // Add optional parameters
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        } else {
            body["max_tokens"] = json!(self.generation.max_tokens);
        }

        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        } else {
            body["temperature"] = json!(self.generation.temperature);
        }

        // Tool choice
        if let Some(tools) = &request.tools {
            let tool_defs: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": &t.function.name,
                            "description": &t.function.description,
                            "parameters": &t.function.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tool_defs);
        }

        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = match tool_choice {
                ToolChoice::None => json!("none"),
                ToolChoice::Auto => json!("auto"),
                ToolChoice::Required => json!("required"),
                ToolChoice::Specific { function, .. } => json!({
                    "type": "function",
                    "function": {
                        "name": function.name,
                    }
                }),
            };
        }

        // Reasoning effort (for models that support it)
        if let Some(effort) = &request.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        } else if let Some(effort) = &self.generation.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }

        body
    }

    /// Convert message to OpenAI format
    fn message_to_openai_format(&self, message: &Message) -> Value {
        let mut msg = json!({
            "role": message.role,
        });

        match &message.content {
            Some(MessageContent::Text(text)) => {
                msg["content"] = json!(text);
            }
            Some(MessageContent::Blocks(blocks)) => {
                let content: Vec<Value> = blocks
                    .iter()
                    .map(|block| match block {
                        crate::base::ContentBlock::Text { text } => json!({
                            "type": "text",
                            "text": text,
                        }),
                        crate::base::ContentBlock::ImageUrl { image_url } => json!({
                            "type": "image_url",
                            "image_url": {
                                "url": &image_url.url,
                                "detail": image_url.detail,
                            }
                        }),
                        crate::base::ContentBlock::InputAudio { input_audio } => json!({
                            "type": "input_audio",
                            "input_audio": {
                                "data": &input_audio.data,
                                "format": &input_audio.format,
                            }
                        }),
                    })
                    .collect();
                msg["content"] = json!(content);
            }
            None => {
                msg["content"] = Value::Null;
            }
        }

        if let Some(tool_calls) = &message.tool_calls {
            let openai_tool_calls: Vec<Value> = tool_calls
                .iter()
                .map(|tc| {
                    json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments,
                        }
                    })
                })
                .collect();
            msg["tool_calls"] = json!(openai_tool_calls);
        }

        if let Some(tool_call_id) = &message.tool_call_id {
            msg["tool_call_id"] = json!(tool_call_id);
        }

        if let Some(name) = &message.name {
            msg["name"] = json!(name);
        }

        msg
    }

    /// Parse response from OpenAI format
    fn parse_response(&self, data: Value) -> Result<LLMResponse> {
        let choice = data
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        let message = choice
            .get("message")
            .ok_or_else(|| ProviderError::InvalidResponse("No message in choice".to_string()))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .map(String::from);

        let reasoning_content = message
            .get("reasoning_content")
            .and_then(|c| c.as_str())
            .map(String::from);

        let tool_calls = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let function = tc.get("function")?;
                        let id = tc.get("id")?.as_str()?;
                        let name = function.get("name")?.as_str()?;
                        let arguments = function
                            .get("arguments")
                            .cloned()
                            .unwrap_or_else(|| json!({}));

                        // Parse arguments if string
                        let parsed_args = if let Some(args_str) = arguments.as_str() {
                            serde_json::from_str(args_str).unwrap_or_else(|_| {
                                json!({ "raw": args_str })
                            })
                        } else {
                            arguments
                        };

                        Some(ToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            arguments: parsed_args,
                            extra_content: None,
                            provider_specific_fields: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|fr| fr.as_str())
            .unwrap_or("stop")
            .to_string();

        let usage = data
            .get("usage")
            .map(|u| TokenUsage {
                prompt_tokens: u
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                completion_tokens: u
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                total_tokens: u
                    .get("total_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                cached_tokens: u
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
            })
            .or_else(|| Some(TokenUsage::default()));

        Ok(LLMResponse {
            content,
            tool_calls,
            finish_reason,
            usage,
            reasoning_content,
            thinking_blocks: None,
        })
    }

    /// Parse streaming delta
    fn parse_stream_delta(&self, data: Value) -> Option<StreamDelta> {
        let choice = data
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())?;

        let delta = choice.get("delta")?;

        let content = delta
            .get("content")
            .and_then(|c| c.as_str())
            .map(String::from);

        let reasoning_content = delta
            .get("reasoning_content")
            .and_then(|c| c.as_str())
            .map(String::from);

        let tool_calls = delta
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let id = tc.get("id").and_then(|v| v.as_str())?;
                        let function = tc.get("function")?;
                        let name = function.get("name")?.as_str()?;
                        let arguments = function.get("arguments")?.as_str()?;

                        Some(ToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            arguments: serde_json::from_str(arguments).unwrap_or_else(|_| {
                                json!({ "partial": arguments })
                            }),
                            extra_content: None,
                            provider_specific_fields: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|fr| fr.as_str())
            .map(String::from);

        Some(StreamDelta {
            content,
            reasoning_content,
            tool_calls,
            finish_reason,
        })
    }
}

/// Streaming delta
#[derive(Debug, Clone, Default)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
}

#[async_trait]
impl LLMProvider for OpenAiCompatProvider {
    fn get_default_model(&self) -> &str {
        &self.model
    }

    fn get_generation_settings(&self) -> &GenerationSettings {
        &self.generation
    }

    async fn chat(&self, request: ChatRequest) -> Result<LLMResponse> {
        let body = self.build_request_body(&request);

        debug!("Sending chat request to {}", self.api_base);

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key));

        // Add extra headers if needed
        req = req.json(&body);

        let response = req.send().await.map_err(|e| {
            error!("HTTP request failed: {}", e);
            ProviderError::Http(e)
        })?;

        let status = response.status();
        let text = response.text().await.map_err(|e| {
            error!("Failed to read response: {}", e);
            ProviderError::Http(e)
        })?;

        if !status.is_success() {
            error!("API error ({}): {}", status, text);
            return Err(ProviderError::InvalidResponse(format!(
                "API error ({}): {}",
                status, text
            )));
        }

        let data: Value = serde_json::from_str(&text).map_err(|e| {
            error!("Failed to parse JSON: {}", e);
            ProviderError::Json(e)
        })?;

        self.parse_response(data)
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
        mut on_delta: StreamCallback,
    ) -> Result<LLMResponse> {
        let mut stream_request = request.clone();
        stream_request.stream = true;

        let body = self.build_request_body(&stream_request);

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                error!("HTTP request failed: {}", e);
                ProviderError::Http(e)
            })?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            error!("API error: {}", text);
            return Err(ProviderError::InvalidResponse(format!(
                "API error: {}",
                text
            )));
        }

        self.handle_stream(response, &mut on_delta).await
    }
}

impl OpenAiCompatProvider {
    /// Handle SSE stream
    async fn handle_stream(
        &self,
        response: Response,
        on_delta: &mut StreamCallback,
    ) -> Result<LLMResponse> {
        let mut full_content = String::new();
        let mut all_tool_calls: Vec<ToolCall> = Vec::new();
        let mut finish_reason = None;

        // Get the response body as text and parse SSE events
        let full_text = response.text().await.map_err(|e| {
            error!("Failed to read response body: {}", e);
            ProviderError::Http(e)
        })?;

        for line in full_text.lines() {
            let line = line.trim();

            if line.starts_with("data: ") {
                let data_str = &line[6..];

                if data_str == "[DONE]" {
                    debug!("Stream completed");
                    break;
                }

                if let Ok(data) = serde_json::from_str::<Value>(data_str) {
                    if let Some(delta) = self.parse_stream_delta(data) {
                        if let Some(content) = delta.content {
                            full_content.push_str(&content);
                            on_delta(content).await;
                        }

                        if !delta.tool_calls.is_empty() {
                            all_tool_calls.extend(delta.tool_calls);
                        }

                        if let Some(reason) = delta.finish_reason {
                            finish_reason = Some(reason);
                        }
                    }
                }
            }
        }

        Ok(LLMResponse {
            content: Some(full_content),
            tool_calls: all_tool_calls,
            finish_reason: finish_reason.unwrap_or_else(|| "stop".to_string()),
            usage: None,
            reasoning_content: None,
            thinking_blocks: None,
        })
    }
}

/// Create provider from config and spec
pub fn create_provider_from_spec(
    api_key: String,
    api_base: String,
    model: String,
    spec: &crate::registry::ProviderSpec,
) -> Box<dyn LLMProvider> {
    Box::new(OpenAiCompatProvider::new(
        api_key,
        api_base,
        model,
        spec.strip_model_prefix,
    ))
}
