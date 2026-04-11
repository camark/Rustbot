//! Skills System
//!
//! This module provides a skill system for RustBot, allowing specialized
//! capabilities to be loaded and executed on demand.

pub mod loader;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Unique identifier for this skill
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this skill does
    pub description: String,
    /// Version string (semver)
    #[serde(default)]
    pub version: String,
    /// Author information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether this skill is enabled by default
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
}

fn default_true() -> bool {
    true
}

impl SkillInfo {
    /// Create a new skill info
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            version: "1.0.0".to_string(),
            author: None,
            tags: vec![],
            enabled_by_default: true,
        }
    }

    /// Set version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set author
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set enabled by default
    pub fn with_enabled_by_default(mut self, enabled: bool) -> Self {
        self.enabled_by_default = enabled;
        self
    }
}

/// Skill trait - all skills must implement this
#[async_trait::async_trait]
pub trait Skill: Send + Sync {
    /// Get skill info
    fn info(&self) -> &SkillInfo;

    /// Get the skill's system prompt
    fn system_prompt(&self) -> &str;

    /// Execute the skill with the given input
    async fn execute(&self, input: SkillInput) -> Result<SkillOutput>;

    /// Validate skill configuration (optional override)
    fn validate_config(&self, _config: &Value) -> Result<()> {
        Ok(())
    }
}

/// Skill input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    /// The main input text/content
    pub content: String,
    /// Additional context
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, Value>,
    /// Configuration overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

impl SkillInput {
    /// Create a new skill input with just content
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            context: HashMap::new(),
            config: None,
        }
    }

    /// Add context
    pub fn with_context(mut self, key: impl Into<String>, value: Value) -> Self {
        self.context.insert(key.into(), value);
        self
    }

    /// Add config
    pub fn with_config(mut self, config: Value) -> Self {
        self.config = Some(config);
        self
    }
}

/// Skill output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    /// The main output content
    pub content: String,
    /// Any metadata produced
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
    /// Whether the skill execution was successful
    pub success: bool,
}

impl SkillOutput {
    /// Create a successful output
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            metadata: HashMap::new(),
            success: true,
        }
    }

    /// Create a failed output
    pub fn failure(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            metadata: HashMap::new(),
            success: false,
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

// ============================================================================
// Built-in Skills
// ============================================================================

/// Memory skill - helps with context retention and recall
pub struct MemorySkill {
    info: SkillInfo,
    system_prompt: String,
}

impl MemorySkill {
    pub fn new() -> Self {
        Self {
            info: SkillInfo::new(
                "memory",
                "Memory Skill",
                "Helps retain and recall information across conversations",
            ),
            system_prompt: String::from(
                "You are a memory assistant. Help track important information, \
                 maintain context across conversations, and recall relevant details \
                 when needed. Focus on:\n\n\
                 1. Identifying key facts and preferences to remember\n\
                 2. Organizing information logically\n\
                 3. Retrieving relevant context when appropriate\n\
                 4. Noting patterns and recurring themes"
            ),
        }
    }
}

impl Default for MemorySkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for MemorySkill {
    fn info(&self) -> &SkillInfo {
        &self.info
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        // Placeholder implementation
        // In full implementation, this would interact with the memory system
        Ok(SkillOutput::success(format!(
            "[Memory Skill] Processed: {}",
            &input.content[..input.content.chars().take(50).count().min(input.content.len())]
        )))
    }
}

/// Code review skill - provides code feedback
pub struct CodeReviewSkill {
    info: SkillInfo,
    system_prompt: String,
}

impl CodeReviewSkill {
    pub fn new() -> Self {
        Self {
            info: SkillInfo::new(
                "code_review",
                "Code Review Skill",
                "Provides thorough code review and feedback",
            ),
            system_prompt: String::from(
                "You are an experienced code reviewer. Provide constructive, \
                 actionable feedback on code quality, focusing on:\n\n\
                 1. Correctness and potential bugs\n\
                 2. Code clarity and maintainability\n\
                 3. Performance considerations\n\
                 4. Security implications\n\
                 5. Testing adequacy\n\
                 6. Architecture and design patterns"
            ),
        }
    }
}

impl Default for CodeReviewSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for CodeReviewSkill {
    fn info(&self) -> &SkillInfo {
        &self.info
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        // Placeholder implementation
        Ok(SkillOutput::success(format!(
            "[Code Review Skill] Analyzing code: {} lines",
            input.content.lines().count()
        )))
    }
}

/// Planning skill - helps with task breakdown and strategy
pub struct PlanningSkill {
    info: SkillInfo,
    system_prompt: String,
}

impl PlanningSkill {
    pub fn new() -> Self {
        Self {
            info: SkillInfo::new(
                "planning",
                "Planning Skill",
                "Helps break down tasks and create execution plans",
            ),
            system_prompt: String::from(
                "You are a strategic planning assistant. Help users:\n\n\
                 1. Understand goals and constraints\n\
                 2. Break down complex tasks into manageable steps\n\
                 3. Identify dependencies and risks\n\
                 4. Estimate time and resource requirements\n\
                 5. Suggest priorities and sequencing\n\
                 6. Track progress and adjust plans as needed"
            ),
        }
    }
}

impl Default for PlanningSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for PlanningSkill {
    fn info(&self) -> &SkillInfo {
        &self.info
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        // Placeholder implementation
        Ok(SkillOutput::success(format!(
            "[Planning Skill] Creating plan for: {}",
            &input.content[..input.content.chars().take(100).count().min(input.content.len())]
        )))
    }
}

// ============================================================================
// Skill Registry
// ============================================================================

/// Skill registry for loading and managing skills
pub struct SkillRegistry {
    skills: RwLock<HashMap<String, Arc<dyn Skill>>>,
    config_path: Option<PathBuf>,
}

impl SkillRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            skills: RwLock::new(HashMap::new()),
            config_path: None,
        }
    }

    /// Create a registry with config path
    pub fn with_config(config_path: impl Into<PathBuf>) -> Self {
        Self {
            skills: RwLock::new(HashMap::new()),
            config_path: Some(config_path.into()),
        }
    }

    /// Create a registry with built-in skills
    pub async fn with_builtins() -> Self {
        let registry = Self::new();
        registry.register_builtin_skills().await;
        registry
    }

    /// Register built-in skills
    pub async fn register_builtin_skills(&self) {
        let memory = Arc::new(MemorySkill::new());
        let code_review = Arc::new(CodeReviewSkill::new());
        let planning = Arc::new(PlanningSkill::new());

        let mut skills = self.skills.write().await;
        skills.insert(memory.info().id.clone(), memory);
        skills.insert(code_review.info().id.clone(), code_review);
        skills.insert(planning.info().id.clone(), planning);

        info!("Registered {} built-in skills", skills.len());
    }

    /// Load user skills from ~/.nanobot/skills/ directory
    /// User skills override built-in skills with the same name (with warning)
    pub async fn load_user_skills(&self, skills_dir: &Path) -> Result<()> {
        use crate::skills::loader::SkillLoader;

        let loader = SkillLoader::new(skills_dir.to_path_buf());
        let user_skills = loader.load_skills().await?;

        if user_skills.is_empty() {
            return Ok(());
        }

        let mut skills = self.skills.write().await;
        for skill in user_skills {
            let id = skill.info().id.clone();
            if skills.contains_key(&id) {
                warn!("User skill '{}' overrides built-in skill", id);
            } else {
                info!("Registered user skill: {}", id);
            }
            // Wrap in Arc for storage
            let skill_arc: Arc<dyn Skill> = Arc::from(skill);
            skills.insert(id, skill_arc);
        }

        info!("Loaded {} user skills", skills.len() - 3); // Subtract built-ins
        Ok(())
    }

    /// Register a skill
    pub async fn register(&self, skill: Arc<dyn Skill>) {
        let mut skills = self.skills.write().await;
        let id = skill.info().id.clone();
        skills.insert(id, skill);
    }

    /// Unregister a skill
    pub async fn unregister(&self, id: &str) -> Option<Arc<dyn Skill>> {
        let mut skills = self.skills.write().await;
        skills.remove(id)
    }

    /// Get a skill by ID
    pub async fn get(&self, id: &str) -> Option<Arc<dyn Skill>> {
        let skills = self.skills.read().await;
        skills.get(id).cloned()
    }

    /// List all registered skills
    pub async fn list(&self) -> Vec<SkillInfo> {
        let skills = self.skills.read().await;
        skills.values().map(|s| s.info().clone()).collect()
    }

    /// Get enabled skills
    pub async fn get_enabled(&self) -> Vec<Arc<dyn Skill>> {
        let skills = self.skills.read().await;
        skills
            .values()
            .filter(|s| s.info().enabled_by_default)
            .cloned()
            .collect()
    }

    /// Execute a skill
    pub async fn execute(&self, id: &str, input: SkillInput) -> Result<SkillOutput> {
        let skill = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Skill not found: {}", id))?;

        debug!("Executing skill '{}'", id);
        skill.execute(input).await
    }

    /// Get skill system prompt for LLM injection
    pub async fn get_skill_prompt(&self, id: &str) -> Option<String> {
        let skills = self.skills.read().await;
        let skill = skills.get(id)?;
        Some(skill.system_prompt().to_string())
    }

    /// Get all skills as LLM tool definitions
    pub async fn get_tool_definitions(&self) -> Vec<nanobot_providers::ToolDefinition> {
        use nanobot_providers::ToolDefinition;
        use serde_json::json;

        let skills = self.skills.read().await;
        skills
            .values()
            .map(|skill| {
                let info = skill.info();
                ToolDefinition::new(
                    info.id.clone(),
                    info.description.clone(),
                    json!({
                        "type": "object",
                        "properties": {
                            "input": {
                                "type": "string",
                                "description": "The input or question for the skill"
                            }
                        },
                        "required": ["input"]
                    }),
                )
            })
            .collect()
    }

    /// Load skills from configuration
    pub async fn load_from_config(&self) -> Result<()> {
        let config_path = self
            .config_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No config path configured"))?;

        if !config_path.exists() {
            debug!("Skill config not found at {:?}", config_path);
            return Ok(());
        }

        let config_content = tokio::fs::read_to_string(config_path)
            .await
            .context("Failed to read skill config")?;

        let config: SkillConfig = serde_json::from_str(&config_content)
            .context("Failed to parse skill config")?;

        self.apply_config(config).await
    }

    /// Apply skill configuration
    pub async fn apply_config(&self, config: SkillConfig) -> Result<()> {
        for skill_config in config.skills {
            if skill_config.enabled {
                info!("Enabling skill: {}", skill_config.id);
                // In full implementation, this would load the skill module
            } else {
                info!("Disabling skill: {}", skill_config.id);
                self.unregister(&skill_config.id).await;
            }
        }

        Ok(())
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Skill configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    /// Skills to load
    #[serde(default)]
    pub skills: Vec<SkillEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    /// Skill ID
    pub id: String,
    /// Whether enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Skill-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

/// Skill manager for high-level operations
pub struct SkillManager {
    registry: Arc<SkillRegistry>,
}

impl SkillManager {
    /// Create a new skill manager
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }

    /// Get available skills
    pub async fn available_skills(&self) -> Vec<SkillInfo> {
        self.registry.list().await
    }

    /// Execute a skill
    pub async fn execute(&self, id: &str, input: SkillInput) -> Result<SkillOutput> {
        self.registry.execute(id, input).await
    }

    /// Get the registry
    pub fn registry(&self) -> &SkillRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_info_builder() {
        let info = SkillInfo::new("test", "Test Skill", "A test skill")
            .with_version("2.0.0")
            .with_author("Test Author")
            .with_tags(vec!["test".into(), "demo".into()]);

        assert_eq!(info.id, "test");
        assert_eq!(info.version, "2.0.0");
        assert_eq!(info.author, Some("Test Author".to_string()));
        assert_eq!(info.tags.len(), 2);
    }

    #[test]
    fn test_skill_input_builder() {
        let input = SkillInput::new("test content")
            .with_context("key", serde_json::json!("value"))
            .with_config(serde_json::json!({"option": true}));

        assert_eq!(input.content, "test content");
        assert!(input.context.contains_key("key"));
        assert!(input.config.is_some());
    }

    #[test]
    fn test_skill_output_creation() {
        let output = SkillOutput::success("result")
            .with_metadata("metric", serde_json::json!(100));

        assert!(output.success);
        assert_eq!(output.content, "result");
        assert!(output.metadata.contains_key("metric"));

        let failure = SkillOutput::failure("error message");
        assert!(!failure.success);
    }

    #[tokio::test]
    async fn test_skill_registry_builtins() {
        let registry = SkillRegistry::with_builtins().await;

        let skills = registry.list().await;
        assert!(!skills.is_empty());

        // Should have memory, code_review, and planning
        assert!(registry.get("memory").await.is_some());
        assert!(registry.get("code_review").await.is_some());
        assert!(registry.get("planning").await.is_some());
    }

    #[tokio::test]
    async fn test_skill_execution() {
        let registry = SkillRegistry::with_builtins().await;

        let input = SkillInput::new("test input");
        let result = registry.execute("memory", input).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.content.contains("[Memory Skill]"));
    }

    #[tokio::test]
    async fn test_skill_manager() {
        let registry = Arc::new(SkillRegistry::with_builtins().await);
        let manager = SkillManager::new(registry);

        let skills = manager.available_skills().await;
        assert!(!skills.is_empty());

        let input = SkillInput::new("planning test");
        let result = manager.execute("planning", input).await;
        assert!(result.is_ok());
    }
}
