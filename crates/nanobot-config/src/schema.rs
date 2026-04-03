//! Configuration schema definitions
//!
//! Compatible with Python nanobot config.json format.
//! Accepts both camelCase and snake_case field names.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Base Types
// ============================================================================

/// Provider backend type
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBackend {
    OpenAiCompat,
    Anthropic,
    AzureOpenai,
    OpenaiCodex,
    GithubCopilot,
}

impl Default for ProviderBackend {
    fn default() -> Self {
        Self::OpenAiCompat
    }
}

// ============================================================================
// Agent Configuration
// ============================================================================

/// Default agent configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefaults {
    /// Workspace directory path (default: ~/.nanobot/workspace)
    #[serde(default = "default_workspace")]
    pub workspace: String,

    /// Model identifier (default: anthropic/claude-opus-4-5)
    #[serde(default = "default_model")]
    pub model: String,

    /// Provider name or "auto" for auto-detection
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Maximum tokens in response
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// Context window size
    #[serde(default = "default_context_window")]
    pub context_window_tokens: u32,

    /// Sampling temperature
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tool iterations
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: u32,

    /// Reasoning effort level (low/medium/high)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,

    /// IANA timezone (default: UTC)
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            workspace: default_workspace(),
            model: default_model(),
            provider: default_provider(),
            max_tokens: default_max_tokens(),
            context_window_tokens: default_context_window(),
            temperature: default_temperature(),
            max_tool_iterations: default_max_tool_iterations(),
            reasoning_effort: None,
            timezone: default_timezone(),
        }
    }
}

fn default_workspace() -> String {
    "~/.nanobot/workspace".to_string()
}

fn default_model() -> String {
    "anthropic/claude-opus-4-5".to_string()
}

fn default_provider() -> String {
    "auto".to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_context_window() -> u32 {
    65_536
}

fn default_temperature() -> f32 {
    0.1
}

fn default_max_tool_iterations() -> u32 {
    40
}

fn default_timezone() -> String {
    "UTC".to_string()
}

/// Agent configuration root
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsConfig {
    #[serde(default)]
    pub defaults: AgentDefaults,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            defaults: AgentDefaults::default(),
        }
    }
}

// ============================================================================
// Provider Configuration
// ============================================================================

/// Single provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: None,
            extra_headers: HashMap::new(),
        }
    }
}

/// All providers configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersConfig {
    #[serde(default)]
    pub custom: ProviderConfig,

    #[serde(default)]
    pub anthropic: ProviderConfig,

    #[serde(default)]
    pub openai: ProviderConfig,

    #[serde(default)]
    pub openrouter: ProviderConfig,

    #[serde(default)]
    pub azure_openai: ProviderConfig,

    #[serde(default)]
    pub deepseek: ProviderConfig,

    #[serde(default)]
    pub groq: ProviderConfig,

    #[serde(default)]
    pub zhipu: ProviderConfig,

    #[serde(default)]
    pub dashscope: ProviderConfig,

    #[serde(default)]
    pub vllm: ProviderConfig,

    #[serde(default)]
    pub ollama: ProviderConfig,

    #[serde(default)]
    pub ovms: ProviderConfig,

    #[serde(default)]
    pub gemini: ProviderConfig,

    #[serde(default)]
    pub moonshot: ProviderConfig,

    #[serde(default)]
    pub minimax: ProviderConfig,

    #[serde(default)]
    pub mistral: ProviderConfig,

    #[serde(default)]
    pub stepfun: ProviderConfig,

    #[serde(default)]
    pub aihubmix: ProviderConfig,

    #[serde(default)]
    pub siliconflow: ProviderConfig,

    #[serde(default)]
    pub volcengine: ProviderConfig,

    #[serde(default)]
    pub volcengine_coding_plan: ProviderConfig,

    #[serde(default)]
    pub byteplus: ProviderConfig,

    #[serde(default)]
    pub byteplus_coding_plan: ProviderConfig,

    #[serde(default)]
    pub openai_codex: ProviderConfig,

    #[serde(default)]
    pub github_copilot: ProviderConfig,
}

// ============================================================================
// Channel Configuration
// ============================================================================

/// Telegram channel configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TelegramConfig {
    /// Bot token for Telegram Bot API
    #[serde(default)]
    pub bot_token: String,

    /// Webhook URL (optional, uses polling if not set)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,

    /// Polling interval in seconds (default: 2)
    #[serde(default = "default_polling_interval")]
    pub polling_interval: u32,
}

fn default_polling_interval() -> u32 {
    2
}

/// Discord channel configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscordConfig {
    /// Bot token for Discord API
    #[serde(default)]
    pub bot_token: String,

    /// Guild (server) ID (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
}

/// Feishu (Lark) channel configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfig {
    /// App ID from Feishu developer console
    #[serde(default)]
    pub app_id: String,

    /// App Secret from Feishu developer console
    #[serde(default)]
    pub app_secret: String,

    /// Verification token for webhook validation
    #[serde(default)]
    pub verification_token: String,
}

/// Channel configuration root
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    /// Send progress updates to channels
    #[serde(default = "default_true")]
    pub send_progress: bool,

    /// Send tool hints to channels
    #[serde(default)]
    pub send_tool_hints: bool,

    /// Maximum retries for sending messages
    #[serde(default = "default_send_max_retries")]
    pub send_max_retries: u32,

    /// Telegram-specific configuration
    #[serde(default)]
    pub telegram: TelegramConfig,

    /// Discord-specific configuration
    #[serde(default)]
    pub discord: DiscordConfig,

    /// Feishu-specific configuration
    #[serde(default)]
    pub feishu: FeishuConfig,

    /// Extra fields for additional channel configs
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

fn default_send_max_retries() -> u32 {
    3
}

// ============================================================================
// Gateway Configuration
// ============================================================================

/// Heartbeat configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_heartbeat_interval")]
    pub interval_s: u64,

    #[serde(default = "default_keep_recent_messages")]
    pub keep_recent_messages: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_s: default_heartbeat_interval(),
            keep_recent_messages: default_keep_recent_messages(),
        }
    }
}

fn default_heartbeat_interval() -> u64 {
    30 * 60 // 30 minutes
}

fn default_keep_recent_messages() -> u32 {
    8
}

/// Gateway configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    #[serde(default)]
    pub host: String,

    #[serde(default = "default_gateway_port")]
    pub port: u16,

    #[serde(default)]
    pub heartbeat: HeartbeatConfig,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: default_gateway_port(),
            heartbeat: HeartbeatConfig::default(),
        }
    }
}

fn default_gateway_port() -> u16 {
    18790
}

// ============================================================================
// Tools Configuration
// ============================================================================

/// Web search configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchConfig {
    #[serde(default = "default_search_provider")]
    pub provider: String,

    #[serde(default)]
    pub api_key: String,

    #[serde(default)]
    pub base_url: String,

    #[serde(default = "default_max_results")]
    pub max_results: u32,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            provider: default_search_provider(),
            api_key: String::new(),
            base_url: String::new(),
            max_results: default_max_results(),
        }
    }
}

fn default_search_provider() -> String {
    "brave".to_string()
}

fn default_max_results() -> u32 {
    5
}

/// Web tools configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebToolsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,

    #[serde(default)]
    pub search: WebSearchConfig,
}

impl Default for WebToolsConfig {
    fn default() -> Self {
        Self {
            proxy: None,
            search: WebSearchConfig::default(),
        }
    }
}

/// Shell exec tool configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecToolConfig {
    #[serde(default = "default_true")]
    pub enable: bool,

    #[serde(default = "default_exec_timeout")]
    pub timeout: u64,

    #[serde(default)]
    pub path_append: String,
}

impl Default for ExecToolConfig {
    fn default() -> Self {
        Self {
            enable: true,
            timeout: default_exec_timeout(),
            path_append: String::new(),
        }
    }
}

fn default_exec_timeout() -> u64 {
    60
}

/// MCP Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "type")]
    pub transport_type: Option<String>,

    #[serde(default)]
    pub command: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: HashMap<String, String>,

    #[serde(default)]
    pub url: String,

    #[serde(default)]
    pub headers: HashMap<String, String>,

    #[serde(default = "default_mcp_tool_timeout")]
    pub tool_timeout: u64,

    #[serde(default = "default_enabled_tools")]
    pub enabled_tools: Vec<String>,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            transport_type: None,
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            url: String::new(),
            headers: HashMap::new(),
            tool_timeout: default_mcp_tool_timeout(),
            enabled_tools: default_enabled_tools(),
        }
    }
}

fn default_mcp_tool_timeout() -> u64 {
    30
}

fn default_enabled_tools() -> Vec<String> {
    vec!["*".to_string()]
}

/// Tools configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsConfig {
    #[serde(default)]
    pub web: WebToolsConfig,

    #[serde(default)]
    pub exec: ExecToolConfig,

    #[serde(default)]
    pub restrict_to_workspace: bool,

    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            web: WebToolsConfig::default(),
            exec: ExecToolConfig::default(),
            restrict_to_workspace: false,
            mcp_servers: HashMap::new(),
        }
    }
}

// ============================================================================
// API Configuration
// ============================================================================

/// OpenAI-compatible API server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    #[serde(default = "default_api_host")]
    pub host: String,

    #[serde(default = "default_api_port")]
    pub port: u16,

    #[serde(default = "default_api_timeout")]
    pub timeout: f64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
            timeout: default_api_timeout(),
        }
    }
}

fn default_api_host() -> String {
    "127.0.0.1".to_string()
}

fn default_api_port() -> u16 {
    8900
}

fn default_api_timeout() -> f64 {
    120.0
}

// ============================================================================
// Root Configuration
// ============================================================================

/// Root configuration for RustBot
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub agents: AgentsConfig,

    #[serde(default)]
    pub channels: ChannelsConfig,

    #[serde(default)]
    pub providers: ProvidersConfig,

    #[serde(default)]
    pub api: ApiConfig,

    #[serde(default)]
    pub gateway: GatewayConfig,

    #[serde(default)]
    pub tools: ToolsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agents: AgentsConfig::default(),
            channels: ChannelsConfig::default(),
            providers: ProvidersConfig::default(),
            api: ApiConfig::default(),
            gateway: GatewayConfig::default(),
            tools: ToolsConfig::default(),
        }
    }
}

impl Config {
    /// Get the workspace path
    pub fn workspace_path(&self) -> PathBuf {
        let path = &self.agents.defaults.workspace;
        if path.starts_with('~') {
            dirs::home_dir()
                .map(|home| home.join(path.trim_start_matches('~')))
                .unwrap_or_else(|| PathBuf::from(path))
        } else {
            PathBuf::from(path)
        }
    }

    /// Get provider config by name
    pub fn get_provider_config(&self, name: &str) -> Option<&ProviderConfig> {
        match name {
            "custom" => Some(&self.providers.custom),
            "anthropic" => Some(&self.providers.anthropic),
            "openai" => Some(&self.providers.openai),
            "openrouter" => Some(&self.providers.openrouter),
            "azure_openai" => Some(&self.providers.azure_openai),
            "deepseek" => Some(&self.providers.deepseek),
            "groq" => Some(&self.providers.groq),
            "zhipu" => Some(&self.providers.zhipu),
            "dashscope" => Some(&self.providers.dashscope),
            "vllm" => Some(&self.providers.vllm),
            "ollama" => Some(&self.providers.ollama),
            "ovms" => Some(&self.providers.ovms),
            "gemini" => Some(&self.providers.gemini),
            "moonshot" => Some(&self.providers.moonshot),
            "minimax" => Some(&self.providers.minimax),
            "mistral" => Some(&self.providers.mistral),
            "stepfun" => Some(&self.providers.stepfun),
            "aihubmix" => Some(&self.providers.aihubmix),
            "siliconflow" => Some(&self.providers.siliconflow),
            "volcengine" => Some(&self.providers.volcengine),
            "volcengine_coding_plan" => Some(&self.providers.volcengine_coding_plan),
            "byteplus" => Some(&self.providers.byteplus),
            "byteplus_coding_plan" => Some(&self.providers.byteplus_coding_plan),
            "openai_codex" => Some(&self.providers.openai_codex),
            "github_copilot" => Some(&self.providers.github_copilot),
            _ => None,
        }
    }
}
