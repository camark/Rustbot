//! Agent loop - the core processing engine

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use nanobot_bus::{InboundMessage, MessageBus, OutboundMessage};
use nanobot_config::Config;
use nanobot_providers::{LLMProvider, LLMResponse, Message, ToolDefinition};

use crate::context::ContextBuilder;
use crate::hooks::AgentHook;
use crate::memory::MemoryManager;
use crate::session::{Session, SessionManager};
use crate::tools::ToolRegistry;
use nanobot_config::ToolsConfig;

/// Agent loop configuration
pub struct AgentLoopConfig {
    pub workspace: PathBuf,
    pub model: String,
    pub max_iterations: usize,
    pub context_window_tokens: u32,
    pub timezone: String,
    pub tools_config: Option<ToolsConfig>,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            workspace: dirs::home_dir()
                .map(|d| d.join(".nanobot").join("workspace"))
                .unwrap_or_else(|| PathBuf::from("~/.nanobot/workspace")),
            model: "anthropic/claude-opus-4-5".to_string(),
            max_iterations: 40,
            context_window_tokens: 65_536,
            timezone: "UTC".to_string(),
            tools_config: None,
        }
    }
}

/// The agent loop is the core processing engine
pub struct AgentLoop {
    bus: Arc<MessageBus>,
    provider: Arc<dyn LLMProvider>,
    workspace: PathBuf,
    model: String,
    max_iterations: usize,
    context_window_tokens: u32,
    tools: Arc<ToolRegistry>,
    sessions: Arc<SessionManager>,
    context: Arc<ContextBuilder>,
    hooks: Arc<RwLock<Vec<Box<dyn AgentHook>>>>,
    running: Arc<RwLock<bool>>,
}

impl AgentLoop {
    /// Create a new agent loop
    pub async fn new(
        bus: MessageBus,
        provider: Arc<dyn LLMProvider>,
        config: AgentLoopConfig,
    ) -> Result<Self, std::io::Error> {
        let sessions = SessionManager::new(&config.workspace)?;
        let context = ContextBuilder::new(&config.workspace, &config.timezone);
        let tools = ToolRegistry::new();

        let mut loop_ = Self {
            bus: Arc::new(bus),
            provider,
            workspace: config.workspace,
            model: config.model,
            max_iterations: config.max_iterations,
            context_window_tokens: config.context_window_tokens,
            tools: Arc::new(tools),
            sessions: Arc::new(sessions),
            context: Arc::new(context),
            hooks: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
        };

        // Register default tools
        loop_.register_default_tools().await;

        Ok(loop_)
    }

    /// Register default tools
    async fn register_default_tools(&mut self) {
        use crate::tools::*;

        // Shell tool - allow access to entire home directory for practical use
        let shell_config = ShellToolConfig::default();
        self.tools.as_ref().register(Box::new(ShellTool::new(self.workspace.to_string_lossy().to_string(), shell_config))).await;

        // Filesystem tools - use home directory as allowed dir for convenience
        // This allows access to Desktop, Documents, etc.
        let allowed_dir = dirs::home_dir()
            .unwrap_or_else(|| self.workspace.clone());

        // Canonicalize allowed_dir to ensure proper path comparison on Windows
        // (canonicalize() returns UNC paths like \\?\C:\ on Windows)
        let allowed_dir_canonical = allowed_dir.canonicalize().unwrap_or_else(|_| allowed_dir.clone());
        info!("Using allowed directory: {} (canonical: {})", allowed_dir.display(), allowed_dir_canonical.display());

        self.tools.as_ref().register(Box::new(ReadFileTool::new(&self.workspace, Some(allowed_dir_canonical.clone())))).await;
        self.tools.as_ref().register(Box::new(WriteFileTool::new(&self.workspace, Some(allowed_dir_canonical.clone())))).await;
        self.tools.as_ref().register(Box::new(EditFileTool::new(&self.workspace, Some(allowed_dir_canonical.clone())))).await;
        self.tools.as_ref().register(Box::new(ListDirTool::new(&self.workspace, Some(allowed_dir_canonical)))).await;

        // Web tools
        let web_search_config = WebSearchConfig::default();
        self.tools.as_ref().register(Box::new(WebSearchTool::new(web_search_config))).await;
        self.tools.as_ref().register(Box::new(WebFetchTool::new(None))).await;

        info!("Registered {} tools", self.tools.as_ref().tool_names().await.len());
    }

    /// Add a hook
    pub async fn add_hook(&self, hook: Box<dyn AgentHook>) {
        self.hooks.write().await.push(hook);
    }

    /// Get the message bus for publishing messages
    pub fn bus(&self) -> &Arc<MessageBus> {
        &self.bus
    }

    /// Run the agent loop
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        info!("Agent loop started");

        while *self.running.read().await {
            // Use tokio::select for cancellation support
            let msg = tokio::select! {
                result = self.bus.as_ref().consume_inbound() => {
                    match result {
                        Ok(msg) => {
                            info!("AgentLoop received inbound message: channel={}, sender={}, chat_id={}, content={}",
                                msg.channel, msg.sender_id, msg.chat_id, msg.content);
                            msg
                        },
                        Err(_) => {
                            warn!("Error consuming inbound message");
                            continue;
                        }
                    }
                }
            };

            // Check for priority commands first
            let raw = msg.content.trim();
            if raw == "/exit" || raw == "/quit" || raw == "exit" || raw == "quit" {
                let response = OutboundMessage::new(&msg.channel, &msg.chat_id, "Goodbye!");
                let _ = self.bus.as_ref().publish_outbound(response).await;
                {
                    let mut running = self.running.write().await;
                    *running = false;
                }
                break;
            }

            // Process message in a spawned task
            let this = self.clone_for_task();
            let msg_clone = msg.clone();
            tokio::spawn(async move {
                if let Err(e) = this.dispatch(msg_clone).await {
                    error!("Error processing message: {}", e);
                }
            });
        }

        info!("Agent loop stopped");
        Ok(())
    }

    /// Clone self for task spawning
    fn clone_for_task(&self) -> Arc<Self> {
        Arc::new(AgentLoop {
            bus: self.bus.clone(),
            provider: self.provider.clone(),
            workspace: self.workspace.clone(),
            model: self.model.clone(),
            max_iterations: self.max_iterations,
            context_window_tokens: self.context_window_tokens,
            tools: self.tools.clone(),
            sessions: self.sessions.clone(),
            context: self.context.clone(),
            hooks: self.hooks.clone(),
            running: self.running.clone(),
        })
    }

    /// Dispatch a message for processing
    async fn dispatch(&self, msg: InboundMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session_key = msg.session_key();
        info!("Dispatch: starting for chat_id={}, session_key={}", msg.chat_id, session_key);

        // Get or create session
        let session_key = msg.session_key();
        let mut session_handle = self.sessions.get_or_create(&session_key).await;
        info!("Dispatch: got session handle for {}", session_key);

        // Save user message to session history first
        let user_msg = serde_json::json!({
            "role": "user",
            "content": msg.content,
        });
        session_handle.add_message(user_msg);

        // Build messages for LLM
        let history = session_handle.get_history(0);
        let mut messages_value = self.context.build_messages(
            history,
            &msg.content,
            &msg.channel,
            &msg.chat_id,
        );

        // Apply token limit truncation
        let memory_manager = MemoryManager::new(
            self.context_window_tokens,
            (self.context_window_tokens as f32 * 0.1) as u32, // Reserve 10% for response
        );
        let (truncated, token_count) = memory_manager.truncate_messages(&messages_value);
        messages_value = truncated;

        info!("Dispatch: using {} messages ({} tokens) for LLM call", messages_value.len(), token_count);

        // Convert Value messages to Message structs
        let messages: Vec<Message> = messages_value
            .into_iter()
            .filter_map(|v| {
                serde_json::from_value(v).ok()
            })
            .collect();

        info!("Dispatch: calling LLM for chat_id={}", msg.chat_id);

        // Get tool definitions for LLM
        let tool_defs = self.tools.get_definitions().await;

        // Call LLM
        let request = nanobot_providers::ChatRequest {
            messages: messages.clone(),
            model: Some(self.model.clone()),
            tools: Some(tool_defs.iter().map(|v| {
                serde_json::from_value(v.clone()).ok()
            }).filter_map(|x| x).collect()),
            max_tokens: Some(8192),
            temperature: Some(0.1),
            reasoning_effort: None,
            tool_choice: Some(nanobot_providers::ToolChoice::Auto),
            stream: msg.wants_streaming(),
        };

        let response = self.provider.chat_with_retry(request).await?;
        info!("Dispatch: LLM response received for chat_id={}", msg.chat_id);

        // Handle response (may need further LLM calls if tool calls exist)
        let mut current_response = response;

        loop {
            // Handle the current response - publish to channel and save to session
            info!("Dispatch: handling response for chat_id={}", msg.chat_id);

            // Check for tool calls before handling response
            let has_tool_calls = current_response.has_tool_calls();

            if has_tool_calls {
                // Execute tool calls and get tool results
                info!("Dispatch: detected {} tool calls, executing...", current_response.tool_calls.len());
                let tool_response = self.execute_tool_calls(&msg, &mut session_handle, &current_response.tool_calls).await?;

                // Save the tool execution results to session (already done in execute_tool_calls)
                // Get another LLM response based on tool results
                if let Some(response) = tool_response {
                    info!("Dispatch: got follow-up response from LLM after tool execution");

                    // Check if the follow-up response also has tool calls - if so, continue the loop
                    if response.has_tool_calls() {
                        info!("Dispatch: follow-up response has {} tool calls, continuing loop", response.tool_calls.len());
                        current_response = response;
                        continue; // Continue the loop to execute more tool calls
                    }

                    // Send the final response to channel
                    if let Some(content) = &response.content {
                        if !content.is_empty() {
                            info!("Dispatch: sending tool result content (len={}): {}", content.len(), content);
                            let outbound = OutboundMessage::new(&msg.channel, &msg.chat_id, content);
                            match self.bus.publish_outbound(outbound).await {
                                Ok(_) => info!("Successfully published tool result to channel"),
                                Err(e) => error!("Failed to publish tool result: {}", e),
                            }
                        } else {
                            warn!("Dispatch: response content is empty");
                        }
                    } else {
                        warn!("Dispatch: response has no content");
                    }
                    // Save final assistant response to session
                    let final_msg = serde_json::json!({
                        "role": "assistant",
                        "content": response.content,
                    });
                    session_handle.add_message(final_msg);
                    session_handle.save()?;
                }
                break;
            } else {
                // No tool calls, just send the response
                self.handle_response(&msg, &mut session_handle, current_response).await?;
                info!("Dispatch: completed for chat_id={}", msg.chat_id);
                break;
            }
        }

        Ok(())
    }

    /// Handle LLM response
    async fn handle_response(
        &self,
        msg: &InboundMessage,
        session: &mut crate::session::SessionHandle,
        response: LLMResponse,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Save assistant message
        let mut assistant_msg = serde_json::json!({
            "role": "assistant",
            "content": response.content,
        });

        // Only include tool_calls if there are any (some providers reject empty arrays)
        if !response.tool_calls.is_empty() {
            let tool_calls: Vec<_> = response.tool_calls.iter().map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                })
            }).collect();
            assistant_msg["tool_calls"] = serde_json::json!(tool_calls);
        }

        session.add_message(assistant_msg);

        // Send response to channel
        if let Some(content) = &response.content {
            if !content.is_empty() {
                let outbound = OutboundMessage::new(&msg.channel, &msg.chat_id, content);
                info!("AgentLoop publishing outbound message: channel={}, chat_id={}, content_len={}",
                    outbound.channel, outbound.chat_id, outbound.content.len());
                match self.bus.publish_outbound(outbound).await {
                    Ok(_) => info!("Successfully published outbound message"),
                    Err(e) => error!("Failed to publish outbound message: {}", e),
                }
            }
        }

        // Execute tool calls if any
        if response.has_tool_calls() {
            // Tool execution will be handled by the caller
        } else {
            // Save session
            session.save()?;
        }

        Ok(())
    }

    /// Execute tool calls
    /// Returns Ok(Some(response)) if tool execution should continue with another LLM call
    /// Returns Ok(None) if processing is complete
    async fn execute_tool_calls(
        &self,
        msg: &InboundMessage,
        session: &mut crate::session::SessionHandle,
        tool_calls: &[nanobot_providers::ToolCall],
    ) -> Result<Option<LLMResponse>, Box<dyn std::error::Error + Send + Sync>> {
        for tool_call in tool_calls {
            info!("Executing tool: {}", tool_call.name);

            let result = self.tools.execute(&tool_call.name, tool_call.arguments.clone()).await;

            let result_value = match result {
                Ok(v) => {
                    // Convert result to string - some providers expect string content
                    v.to_string()
                },
                Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string()),
            };

            // Save tool response to session
            let tool_msg = serde_json::json!({
                "role": "tool",
                "content": result_value,
                "tool_call_id": tool_call.id,
            });
            info!("Tool response saved to session: role=tool, id={}, content_len={}", tool_call.id, result_value.len());
            session.add_message(tool_msg);
        }

        // After executing all tool calls, make another LLM call to get the final response
        let history = session.get_history(0);
        info!("=== DEBUG: Session History ({} messages) ===", history.len());
        for (i, h) in history.iter().enumerate() {
            let role = h.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
            let content_len = h.get("content").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0);
            info!("  [{}] role={}, content_len={}", i, role, content_len);
        }

        let messages_value = self.context.build_messages_from_history(history);
        info!("=== DEBUG: Messages to send to LLM ({} messages) ===", messages_value.len());
        for (i, m) in messages_value.iter().enumerate() {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
            let content_preview = m.get("content")
                .and_then(|v| v.as_str())
                .map(|s| {
                    if s.len() > 100 {
                        // Use char-based slicing to avoid UTF-8 boundary issues
                        let truncated: String = s.chars().take(100).collect();
                        format!("{}...", truncated)
                    } else {
                        s.to_string()
                    }
                })
                .unwrap_or_else(|| "[non-string content]".to_string());
            info!("  [{}] role={}, content_preview={}", i, role, content_preview);
        }
        info!("=== END DEBUG ===");

        // Convert Value messages to Message structs
        let messages: Vec<Message> = messages_value
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        // Get tool definitions for LLM
        let tool_defs = self.tools.get_definitions().await;

        let request = nanobot_providers::ChatRequest {
            messages,
            model: Some(self.model.clone()),
            tools: Some(tool_defs.iter().map(|v| {
                serde_json::from_value(v.clone()).ok()
            }).filter_map(|x| x).collect()),
            max_tokens: Some(8192),
            temperature: Some(0.1),
            reasoning_effort: None,
            tool_choice: Some(nanobot_providers::ToolChoice::Auto),
            stream: false,
        };

        let response = self.provider.chat_with_retry(request).await?;
        info!("=== DEBUG: LLM Response ===");
        info!("  content: {:?}", response.content);
        info!("  tool_calls: {:?}", response.tool_calls);
        info!("=== END DEBUG ===");
        Ok(Some(response))
    }

    /// Stop the agent loop
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Agent loop stopping");
    }

    /// Check if running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Clone for AgentLoop {
    fn clone(&self) -> Self {
        Self {
            bus: self.bus.clone(),
            provider: self.provider.clone(),
            workspace: self.workspace.clone(),
            model: self.model.clone(),
            max_iterations: self.max_iterations,
            context_window_tokens: self.context_window_tokens,
            tools: self.tools.clone(),
            sessions: self.sessions.clone(),
            context: self.context.clone(),
            hooks: self.hooks.clone(),
            running: self.running.clone(),
        }
    }
}
