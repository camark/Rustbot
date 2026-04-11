//! Agent loop - the core processing engine

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use nanobot_bus::{InboundMessage, MessageBus, OutboundMessage};
use nanobot_config::Config;
use nanobot_providers::{LLMProvider, LLMResponse, Message};

use crate::context::ContextBuilder;
use crate::hooks::AgentHook;
use crate::memory::MemoryManager;
use crate::mcp::client::{McpClient, McpClientConfig};
use crate::mcp::tools::McpToolIntegration;
use crate::session::SessionManager;
use crate::skills::{SkillRegistry, SkillInput};
use crate::tools::ToolRegistry;
use nanobot_config::{McpServerConfig, ToolsConfig};

/// Agent loop configuration
pub struct AgentLoopConfig {
    pub workspace: PathBuf,
    pub model: String,
    pub max_iterations: usize,
    pub context_window_tokens: u32,
    pub timezone: String,
    pub tools_config: Option<ToolsConfig>,
    pub skills_enabled: bool,
}

impl AgentLoopConfig {
    /// Create config from full Config
    pub fn from_full_config(config: &Config, model: Option<String>) -> Self {
        Self {
            workspace: config.workspace_path(),
            model: model.unwrap_or_else(|| config.agents.defaults.model.clone()),
            max_iterations: config.agents.defaults.max_tool_iterations as usize,
            context_window_tokens: config.agents.defaults.context_window_tokens,
            timezone: config.agents.defaults.timezone.clone(),
            tools_config: Some(config.tools.clone()),
            skills_enabled: config.skills.enabled,
        }
    }
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
            skills_enabled: false,
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
    skills: Option<Arc<SkillRegistry>>,
    sessions: Arc<SessionManager>,
    context: Arc<ContextBuilder>,
    hooks: Arc<RwLock<Vec<Box<dyn AgentHook>>>>,
    running: Arc<RwLock<bool>>,
    /// MCP clients and tool integrations
    mcp_clients: Vec<Arc<McpClient>>,
    mcp_integrations: Vec<Arc<McpToolIntegration>>,
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

        // Initialize skills if enabled
        let skills = if config.skills_enabled {
            let registry = SkillRegistry::with_builtins().await;
            // Load user skills from ~/.nanobot/skills/
            let skills_dir = config.workspace.join("skills");
            if let Err(e) = registry.load_user_skills(&skills_dir).await {
                warn!("Failed to load user skills: {}", e);
            }
            Some(Arc::new(registry))
        } else {
            None
        };

        // Initialize MCP clients if configured
        let (mcp_clients, mcp_integrations) = Self::initialize_mcp_clients(&config.tools_config).await;

        let mut loop_ = Self {
            bus: Arc::new(bus),
            provider,
            workspace: config.workspace,
            model: config.model,
            max_iterations: config.max_iterations,
            context_window_tokens: config.context_window_tokens,
            tools: Arc::new(tools),
            skills,
            sessions: Arc::new(sessions),
            context: Arc::new(context),
            hooks: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            mcp_clients,
            mcp_integrations,
        };

        // Register default tools (including MCP tools)
        loop_.register_default_tools().await;

        Ok(loop_)
    }

    /// Initialize MCP clients from configuration
    async fn initialize_mcp_clients(tools_config: &Option<ToolsConfig>) -> (Vec<Arc<McpClient>>, Vec<Arc<McpToolIntegration>>) {
        let mut clients = Vec::new();
        let mut integrations = Vec::new();

        let config = match tools_config {
            Some(cfg) => cfg,
            None => return (clients, integrations),
        };

        if config.mcp_servers.is_empty() {
            return (clients, integrations);
        }

        for (name, server_config) in &config.mcp_servers {
            info!("Initializing MCP client: {}", name);

            let client = Self::create_mcp_client(name, server_config);
            match client.connect().await {
                Ok(()) => {
                    info!("MCP client '{}' connected successfully", name);

                    // Create tool integration for this client
                    let integration = McpToolIntegration::new(client.clone());
                    match integration.initialize().await {
                        Ok(tools) => {
                            info!("MCP client '{}' discovered {} tools", name, tools.len());
                            clients.push(client);
                            integrations.push(Arc::new(integration));
                        }
                        Err(e) => {
                            warn!("Failed to discover tools from MCP client '{}': {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to connect MCP client '{}': {}", name, e);
                }
            }
        }

        (clients, integrations)
    }

    /// Create MCP client from configuration
    fn create_mcp_client(_name: &str, config: &McpServerConfig) -> Arc<McpClient> {
        use crate::mcp::transport::TransportConfig;
        use std::sync::Arc;

        let transport_type = config.transport_type.as_deref().unwrap_or("stdio");

        let client_config = if transport_type == "sse" && !config.url.is_empty() {
            // SSE transport
            McpClientConfig {
                transport: TransportConfig::Sse {
                    url: config.url.clone(),
                    headers: config.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                },
                timeout_secs: config.tool_timeout,
                auto_reconnect: true,
                reconnect_delay_secs: 5,
            }
        } else {
            // Stdio transport (default)
            McpClientConfig {
                transport: TransportConfig::Stdio {
                    command: config.command.clone(),
                    args: config.args.clone(),
                    env: config.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                },
                timeout_secs: config.tool_timeout,
                auto_reconnect: true,
                reconnect_delay_secs: 5,
            }
        };

        info!("Creating MCP client with transport: {}", transport_type);
        Arc::new(McpClient::new(client_config))
    }

    /// Register default tools
    async fn register_default_tools(&mut self) {
        use crate::tools::*;

        // Shell tool - allow access to entire home directory for practical use
        let shell_config = ShellToolConfig::default();
        self.tools.as_ref().register(Arc::new(ShellTool::new(self.workspace.to_string_lossy().to_string(), shell_config))).await;

        // Filesystem tools - use home directory as allowed dir for convenience
        // This allows access to Desktop, Documents, etc.
        let allowed_dir = dirs::home_dir()
            .unwrap_or_else(|| self.workspace.clone());

        // Canonicalize allowed_dir to ensure proper path comparison on Windows
        // (canonicalize() returns UNC paths like \\?\C:\ on Windows)
        let allowed_dir_canonical = allowed_dir.canonicalize().unwrap_or_else(|_| allowed_dir.clone());
        info!("Using allowed directory: {} (canonical: {})", allowed_dir.display(), allowed_dir_canonical.display());

        self.tools.as_ref().register(Arc::new(ReadFileTool::new(&self.workspace, Some(allowed_dir_canonical.clone())))).await;
        self.tools.as_ref().register(Arc::new(WriteFileTool::new(&self.workspace, Some(allowed_dir_canonical.clone())))).await;
        self.tools.as_ref().register(Arc::new(EditFileTool::new(&self.workspace, Some(allowed_dir_canonical.clone())))).await;
        self.tools.as_ref().register(Arc::new(ListDirTool::new(&self.workspace, Some(allowed_dir_canonical)))).await;

        // Web tools
        let web_search_config = WebSearchConfig::default();
        self.tools.as_ref().register(Arc::new(WebSearchTool::new(web_search_config))).await;
        self.tools.as_ref().register(Arc::new(WebFetchTool::new(None))).await;

        // Register MCP tools from all connected MCP clients
        self.register_mcp_tools().await;

        info!("Registered {} tools", self.tools.as_ref().tool_names().await.len());
    }

    /// Register MCP tools to the internal ToolRegistry
    async fn register_mcp_tools(&mut self) {
        if self.mcp_integrations.is_empty() {
            return;
        }

        for integration in &self.mcp_integrations {
            let mcp_tools = integration.get_all_tools().await;
            let count = mcp_tools.len();

            for tool in mcp_tools {
                self.tools.as_ref().register_arc(tool).await;
            }

            info!("Registered {} MCP tools", count);
        }
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
            skills: self.skills.clone(),
            sessions: self.sessions.clone(),
            context: self.context.clone(),
            hooks: self.hooks.clone(),
            running: self.running.clone(),
            mcp_clients: self.mcp_clients.clone(),
            mcp_integrations: self.mcp_integrations.clone(),
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

        // Detect and execute skills if enabled
        if let Some(skills) = &self.skills {
            if let Some(skill_result) = self.detect_and_execute_skill(skills, &msg.content).await? {
                info!("Skill executed, result len={}", skill_result.len());
                // Add skill result as system context for the LLM
                let skill_msg = serde_json::json!({
                    "role": "system",
                    "content": format!("[Skill Context]: {}", skill_result),
                });
                session_handle.add_message(skill_msg);
            }
        }

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

        // Get tool definitions (tools + skills)
        let mut tool_defs: Vec<serde_json::Value> = self.tools.get_definitions().await;

        // Add skills as tools if enabled
        if let Some(skills) = &self.skills {
            let skill_tools = skills.get_tool_definitions().await;
            let skill_count = skill_tools.len();
            tool_defs.extend(skill_tools.into_iter().map(|t| serde_json::to_value(t).unwrap()));
            info!("Registered {} skills as LLM tools", skill_count);
        }

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
                // Save assistant message with tool calls to session FIRST
                // (required for DeepSeek and other providers that validate tool call sequences)
                let assistant_msg_with_tool_calls = serde_json::json!({
                    "role": "assistant",
                    "content": current_response.content,
                    "tool_calls": current_response.tool_calls.iter().map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments,
                            }
                        })
                    }).collect::<Vec<_>>(),
                });
                info!("Saving assistant message with tool_calls: {}",
                    serde_json::to_string(&assistant_msg_with_tool_calls).unwrap_or_default());
                session_handle.add_message(assistant_msg_with_tool_calls);

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
        _msg: &InboundMessage,
        session: &mut crate::session::SessionHandle,
        tool_calls: &[nanobot_providers::ToolCall],
    ) -> Result<Option<LLMResponse>, Box<dyn std::error::Error + Send + Sync>> {
        // Check if any tool calls are skill calls
        let skills = self.skills.as_ref();

        for tool_call in tool_calls {
            info!("Executing tool: {}", tool_call.name);

            // Check if this is a skill call
            let is_skill = skills.is_some_and(|s| {
                futures::executor::block_on(async {
                    s.get(&tool_call.name).await.is_some()
                })
            });

            let result_value = if is_skill {
                // Execute skill - inject skill's system prompt into the next LLM call
                // The skill prompt will be added to the system message
                info!("Skill call detected: {}", tool_call.name);

                // Get skill prompt and add as context
                if let Some(skill_registry) = skills {
                    let skill_prompt = skill_registry.get_skill_prompt(&tool_call.name).await;
                    if let Some(prompt) = skill_prompt {
                        // Add skill prompt as a system message for the next LLM call
                        let skill_system_msg = serde_json::json!({
                            "role": "system",
                            "content": format!("[Skill Context: {}]\n{}", tool_call.name, prompt),
                        });
                        session.add_message(skill_system_msg);
                    }
                }

                // Return a placeholder - the actual skill execution happens via LLM
                format!("Skill '{}' executed. See system context for details.", tool_call.name)
            } else {
                // Regular tool execution
                let result = self.tools.execute(&tool_call.name, tool_call.arguments.clone()).await;
                match result {
                    Ok(v) => v.to_string(),
                    Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string()),
                }
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
        let messages_value = self.context.build_messages_from_history(history);

        // Convert Value messages to Message structs
        let messages: Vec<Message> = messages_value
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        // Get tool definitions (tools + skills)
        let mut tool_defs: Vec<serde_json::Value> = self.tools.get_definitions().await;
        if let Some(skill_registry) = skills {
            let skill_tools = skill_registry.get_tool_definitions().await;
            let skill_count = skill_tools.len();
            tool_defs.extend(skill_tools.into_iter().map(|t| serde_json::to_value(t).unwrap()));
            if skill_count > 0 {
                info!("Added {} skills to LLM tool definitions", skill_count);
            }
        }

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
        Ok(Some(response))
    }

    /// Detect and execute skills from user input
    /// Returns Some(skill_output) if a skill was triggered, None otherwise
    async fn detect_and_execute_skill(
        &self,
        registry: &SkillRegistry,
        content: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        // Check for /skill: or /skills: command (colon syntax)
        if let Some(rest) = content.strip_prefix("/skill:").or_else(|| content.strip_prefix("/skills:")) {
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() >= 2 {
                let skill_id = parts[0].trim();
                let skill_input = parts[1].trim();

                let input = SkillInput::new(skill_input);
                match registry.execute(skill_id, input).await {
                    Ok(output) => return Ok(Some(output.content)),
                    Err(e) => return Ok(Some(format!("Skill error: {}", e))),
                }
            }
        }

        // Check for /skill <name> (space syntax, Claude Code style)
        if let Some(rest) = content.strip_prefix("/skill ") {
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() >= 2 {
                let skill_id = parts[0].trim();
                let skill_input = parts[1].trim();

                let input = SkillInput::new(skill_input);
                match registry.execute(skill_id, input).await {
                    Ok(output) => return Ok(Some(output.content)),
                    Err(e) => return Ok(Some(format!("Skill error: {}", e))),
                }
            }
        }

        // Check for specific skill commands (Claude Code style)
        // /memory, /code-review, /planning
        if let Some(input) = content.strip_prefix("/memory ") {
            let input = SkillInput::new(input.trim());
            match registry.execute("memory", input).await {
                Ok(output) => return Ok(Some(output.content)),
                Err(e) => return Ok(Some(format!("Skill error: {}", e))),
            }
        }

        if let Some(input) = content.strip_prefix("/code-review ") {
            let input = SkillInput::new(input.trim());
            match registry.execute("code_review", input).await {
                Ok(output) => return Ok(Some(output.content)),
                Err(e) => return Ok(Some(format!("Skill error: {}", e))),
            }
        }

        if let Some(input) = content.strip_prefix("/planning ") {
            let input = SkillInput::new(input.trim());
            match registry.execute("planning", input).await {
                Ok(output) => return Ok(Some(output.content)),
                Err(e) => return Ok(Some(format!("Skill error: {}", e))),
            }
        }

        Ok(None)
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
            skills: self.skills.clone(),
            sessions: self.sessions.clone(),
            context: self.context.clone(),
            hooks: self.hooks.clone(),
            running: self.running.clone(),
            mcp_clients: self.mcp_clients.clone(),
            mcp_integrations: self.mcp_integrations.clone(),
        }
    }
}
