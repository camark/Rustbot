//! Subagent System
//!
//! This module provides subagent functionality, allowing the main agent to
//! delegate tasks to specialized sub-agents with specific roles and capabilities.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │   Main Agent    │
//! │   (coordinator) │
//! └────────┬────────┘
//!          │ delegate
//!          ▼
//! ┌─────────────────────────────────────────┐
//! │           Subagent Router               │
//! │  - Route tasks to appropriate agent     │
//! │  - Collect and aggregate results        │
//! └────────┬────────────────────────────────┘
//!          │
//!    ┌─────┴─────┬──────────┬──────────┐
//!    ▼           ▼          ▼          ▼
//! ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
//! │ Code   │ │ Review │ │ Plan   │ │ Custom │
//! │ Agent  │ │ Agent  │ │ Agent  │ │ Agent  │
//! └────────┘ └────────┘ └────────┘ └────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::session::Session;

/// Subagent specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSpec {
    /// Unique identifier for this subagent
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of capabilities
    pub description: String,
    /// System prompt override for this subagent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Model to use (defaults to main agent model if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Maximum iterations for this subagent
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// Whether this subagent can use tools
    #[serde(default = "default_true")]
    pub can_use_tools: bool,
    /// Tool restrictions (if can_use_tools is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Temperature setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

fn default_max_iterations() -> usize {
    20
}

fn default_true() -> bool {
    true
}

impl SubagentSpec {
    /// Create a new subagent spec
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            system_prompt: None,
            model: None,
            max_iterations: default_max_iterations(),
            can_use_tools: true,
            allowed_tools: None,
            temperature: None,
        }
    }

    /// Set system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set max iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set allowed tools
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }
}

/// Built-in subagent types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuiltinSubagent {
    /// Code generation subagent
    Code {
        language: Option<String>,
        framework: Option<String>,
    },
    /// Code review subagent
    Review {
        focus_areas: Vec<String>,
    },
    /// Planning and analysis subagent
    Planning {
        analysis_depth: AnalysisDepth,
    },
    /// Research subagent
    Research {
        sources: Vec<String>,
    },
    /// Custom subagent with specific system prompt
    Custom {
        system_prompt: String,
    },
}

/// Analysis depth for planning subagent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDepth {
    Quick,
    #[default]
    Standard,
    Deep,
}

impl BuiltinSubagent {
    /// Get the spec for a builtin subagent
    pub fn spec(&self, id: &str) -> SubagentSpec {
        match self {
            BuiltinSubagent::Code { language, framework } => {
                let mut desc = String::from("Specialized code generation subagent");
                if let Some(lang) = language {
                    desc.push_str(&format!(" for {}", lang));
                }
                if let Some(fw) = framework {
                    desc.push_str(&format!(" using {}", fw));
                }

                SubagentSpec::new(id, "Code Agent", desc)
                    .with_system_prompt(self.code_system_prompt())
            }
            BuiltinSubagent::Review { focus_areas } => {
                let mut desc = String::from("Code review subagent");
                if !focus_areas.is_empty() {
                    desc.push_str(&format!(" focusing on: {}", focus_areas.join(", ")));
                }

                SubagentSpec::new(id, "Review Agent", desc)
                    .with_system_prompt(self.review_system_prompt())
            }
            BuiltinSubagent::Planning { analysis_depth } => {
                let desc = match analysis_depth {
                    AnalysisDepth::Quick => "Quick planning and analysis subagent".to_string(),
                    AnalysisDepth::Standard => "Standard planning and analysis subagent".to_string(),
                    AnalysisDepth::Deep => "Deep analysis and strategic planning subagent".to_string(),
                };

                SubagentSpec::new(id, "Planning Agent", desc)
                    .with_system_prompt(self.planning_system_prompt())
                    .with_max_iterations(30)
            }
            BuiltinSubagent::Research { sources } => {
                let desc = format!("Research subagent with {} sources", sources.len());

                SubagentSpec::new(id, "Research Agent", desc)
                    .with_system_prompt(self.research_system_prompt())
            }
            BuiltinSubagent::Custom { system_prompt } => {
                SubagentSpec::new(id, "Custom Agent", "Custom specialized subagent")
                    .with_system_prompt(system_prompt.clone())
            }
        }
    }

    fn code_system_prompt(&self) -> String {
        String::from(
            "You are a specialized code generation assistant. Your task is to write \
             high-quality, well-tested, and maintainable code. Follow these guidelines:\n\n\
             1. Write idiomatic code for the target language/framework\n\
             2. Include appropriate error handling\n\
             3. Add clear comments for complex logic\n\
             4. Follow established conventions and best practices\n\
             5. Consider security implications\n\
             6. Suggest tests when appropriate"
        )
    }

    fn review_system_prompt(&self) -> String {
        String::from(
            "You are an experienced code reviewer. Your task is to provide thorough, \
             constructive feedback on code quality. Focus on:\n\n\
             1. Correctness and potential bugs\n\
             2. Code clarity and readability\n\
             3. Architecture and design patterns\n\
             4. Performance considerations\n\
             5. Security vulnerabilities\n\
             6. Test coverage\n\n\
             Provide specific, actionable suggestions with code examples when helpful."
        )
    }

    fn planning_system_prompt(&self) -> String {
        String::from(
            "You are a strategic planning assistant. Help break down complex tasks into \
             manageable steps and identify potential challenges.\n\n\
             1. Understand the overall goal and constraints\n\
             2. Break down into logical phases/steps\n\
             3. Identify dependencies and risks\n\
             4. Suggest resource requirements\n\
             5. Provide time estimates when possible\n\
             6. Highlight critical decision points"
        )
    }

    fn research_system_prompt(&self) -> String {
        String::from(
            "You are a research assistant. Gather and synthesize information from \
             multiple sources to provide comprehensive answers.\n\n\
             1. Search for relevant information\n\
             2. Evaluate source credibility\n\
             3. Cross-reference facts when possible\n\
             4. Note any uncertainties or conflicting information\n\
             5. Provide citations for key claims"
        )
    }
}

/// Delegation request from main agent to subagent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRequest {
    /// Target subagent ID
    pub subagent_id: String,
    /// Task description
    pub task: String,
    /// Context from main agent
    pub context: Value,
    /// Expected output format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
}

/// Result from subagent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    /// Subagent that executed the task
    pub subagent_id: String,
    /// Task completion status
    pub status: SubagentStatus,
    /// Result content
    pub content: String,
    /// Any artifacts (file paths, etc.)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
    /// Token usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// Subagent execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Success,
    PartialSuccess,
    Failed,
    Timeout,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

/// Subagent registry
pub struct SubagentRegistry {
    subagents: RwLock<HashMap<String, SubagentSpec>>,
}

impl SubagentRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            subagents: RwLock::new(HashMap::new()),
        }
    }

    /// Create registry with builtin subagents
    pub async fn with_builtins() -> Self {
        let registry = Self::new();

        // Register builtin subagents
        let builtins = vec![
            BuiltinSubagent::Code {
                language: None,
                framework: None,
            }.spec("code"),
            BuiltinSubagent::Review {
                focus_areas: vec!["correctness".into(), "security".into()],
            }.spec("review"),
            BuiltinSubagent::Planning {
                analysis_depth: AnalysisDepth::Standard,
            }.spec("planning"),
        ];

        for spec in builtins {
            registry.register(spec).await;
        }

        registry
    }

    /// Register a subagent spec
    pub async fn register(&self, spec: SubagentSpec) {
        let mut subagents = self.subagents.write().await;
        subagents.insert(spec.id.clone(), spec);
    }

    /// Unregister a subagent
    pub async fn unregister(&self, id: &str) -> Option<SubagentSpec> {
        let mut subagents = self.subagents.write().await;
        subagents.remove(id)
    }

    /// Get a subagent spec by ID
    pub async fn get(&self, id: &str) -> Option<SubagentSpec> {
        let subagents = self.subagents.read().await;
        subagents.get(id).cloned()
    }

    /// List all registered subagents
    pub async fn list(&self) -> Vec<SubagentSpec> {
        let subagents = self.subagents.read().await;
        subagents.values().cloned().collect()
    }

    /// Check if a subagent exists
    pub async fn has(&self, id: &str) -> bool {
        let subagents = self.subagents.read().await;
        subagents.contains_key(id)
    }
}

impl Default for SubagentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Subagent manager for handling delegation
pub struct SubagentManager {
    registry: Arc<SubagentRegistry>,
    #[allow(dead_code)]
    session: Arc<RwLock<Session>>,
}

impl SubagentManager {
    /// Create a new subagent manager
    pub fn new(registry: Arc<SubagentRegistry>, session: Arc<RwLock<Session>>) -> Self {
        Self { registry, session }
    }

    /// Delegate a task to a subagent
    pub async fn delegate(&self, request: DelegationRequest) -> Result<SubagentResult> {
        let spec = self
            .registry
            .get(&request.subagent_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Subagent not found: {}", request.subagent_id))?;

        info!(
            "Delegating to subagent '{}': {}",
            request.subagent_id, request.task
        );

        // Execute the subagent task
        // For now, this is a placeholder - actual implementation would:
        // 1. Create a new agent loop with the subagent's configuration
        // 2. Run the task with the subagent's system prompt
        // 3. Collect results and return

        let result = self.execute_subagent(&spec, &request).await?;

        Ok(result)
    }

    /// Execute a subagent task
    async fn execute_subagent(
        &self,
        spec: &SubagentSpec,
        request: &DelegationRequest,
    ) -> Result<SubagentResult> {
        // Placeholder implementation
        // In full implementation, this would:
        // 1. Spawn a new agent loop with spec configuration
        // 2. Pass the task and context
        // 3. Monitor execution
        // 4. Return formatted result

        debug!(
            "Executing subagent '{}' task: {}",
            spec.id, request.task
        );

        // Simulated result for placeholder
        Ok(SubagentResult {
            subagent_id: spec.id.clone(),
            status: SubagentStatus::Success,
            content: format!(
                "[Subagent {}] Task completed: {}",
                spec.name, request.task
            ),
            artifacts: vec![],
            token_usage: Some(TokenUsage {
                input_tokens: 100,
                output_tokens: 200,
                total_tokens: 300,
            }),
        })
    }

    /// Get available subagents
    pub async fn available_subagents(&self) -> Vec<SubagentSpec> {
        self.registry.list().await
    }

    /// Check if a subagent is available
    pub async fn has_subagent(&self, id: &str) -> bool {
        self.registry.has(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;

    fn create_test_session() -> Arc<RwLock<Session>> {
        Arc::new(RwLock::new(Session::new("test-session", "test-channel", "test-chat")))
    }

    #[test]
    fn test_subagent_spec_builder() {
        let spec = SubagentSpec::new("test", "Test Agent", "A test agent")
            .with_system_prompt("You are a test agent")
            .with_model("test-model")
            .with_max_iterations(10);

        assert_eq!(spec.id, "test");
        assert_eq!(spec.name, "Test Agent");
        assert_eq!(spec.max_iterations, 10);
        assert!(spec.system_prompt.is_some());
    }

    #[test]
    fn test_builtin_subagent_specs() {
        let code_spec = BuiltinSubagent::Code {
            language: Some("Rust".into()),
            framework: Some("Tokio".into()),
        }.spec("code_agent");

        assert!(code_spec.system_prompt.unwrap().contains("code generation"));

        let review_spec = BuiltinSubagent::Review {
            focus_areas: vec!["security".into()],
        }.spec("review_agent");

        assert!(review_spec.system_prompt.unwrap().contains("code reviewer"));
    }

    #[tokio::test]
    async fn test_subagent_registry() {
        let registry = SubagentRegistry::new();

        let spec = SubagentSpec::new("test", "Test", "A test agent");
        registry.register(spec).await;

        assert!(registry.has("test").await);
        assert!(!registry.has("nonexistent").await);

        let retrieved = registry.get("test").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test");
    }

    #[tokio::test]
    async fn test_builtin_registry() {
        let registry = SubagentRegistry::with_builtins().await;

        let subagents = registry.list().await;
        assert!(!subagents.is_empty());

        // Should have code, review, and planning subagents
        assert!(registry.has("code").await);
        assert!(registry.has("review").await);
        assert!(registry.has("planning").await);
    }
}
