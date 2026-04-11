//! Skill Loader - Load user skills from ~/.nanobot/skills/ directory

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::{Skill, SkillInfo, SkillInput, SkillOutput};

/// Skill manifest parsed from skill.md
#[derive(Debug, Clone, Deserialize)]
pub struct SkillManifest {
    /// Skill ID (used as tool name)
    pub name: String,
    /// Skill description (used as tool description)
    pub description: String,
    /// Version string
    #[serde(default)]
    pub version: String,
    /// Author information
    #[serde(default)]
    pub author: Option<String>,
}

impl SkillManifest {
    /// Parse manifest from markdown content with frontmatter
    pub fn from_markdown(content: &str) -> Result<Self> {
        // Extract frontmatter between --- delimiters
        if !content.starts_with("---") {
            anyhow::bail!("skill.md must start with '---' frontmatter delimiter");
        }

        let end_delimiter = content[3..]
            .find("\n---")
            .map(|pos| pos + 3)
            .ok_or_else(|| anyhow::anyhow!("skill.md frontmatter not closed"))?;

        let frontmatter = &content[4..end_delimiter];
        let manifest: SkillManifest = serde_yaml::from_str(frontmatter)
            .context("Failed to parse skill.md frontmatter")?;

        if manifest.name.is_empty() {
            anyhow::bail!("skill.md 'name' field is required");
        }
        if manifest.description.is_empty() {
            anyhow::bail!("skill.md 'description' field is required");
        }

        Ok(manifest)
    }

    /// Get the body content (after frontmatter)
    pub fn get_body(content: &str) -> Result<&str> {
        if !content.starts_with("---") {
            return Ok(content);
        }

        let end_delimiter = content[3..]
            .find("\n---")
            .map(|pos| pos + 6) // Skip "\n---"
            .ok_or_else(|| anyhow::anyhow!("skill.md frontmatter not closed"))?;

        Ok(&content[end_delimiter..])
    }
}

/// User skill that wraps the manifest and system prompt
pub struct UserSkill {
    info: SkillInfo,
    system_prompt: String,
    #[allow(dead_code)]
    script_path: Option<PathBuf>,
}

impl UserSkill {
    /// Create a new user skill from manifest and body
    pub fn new(manifest: SkillManifest, body: String, script_path: Option<PathBuf>) -> Self {
        Self {
            info: SkillInfo::new(
                &manifest.name,
                &manifest.name,
                &manifest.description,
            )
            .with_version(manifest.version)
            .with_author(manifest.author.unwrap_or_default())
            .with_enabled_by_default(true),
            system_prompt: body,
            script_path,
        }
    }

    /// Get the system prompt for this skill
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }
}

#[async_trait::async_trait]
impl Skill for UserSkill {
    fn info(&self) -> &SkillInfo {
        &self.info
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    async fn execute(&self, _input: SkillInput) -> Result<SkillOutput> {
        // User skills return their system prompt as context for the LLM
        // The actual execution happens in AgentLoop when it receives the tool call
        Ok(SkillOutput::success(format!(
            "[Skill: {}] {}",
            self.info.name,
            self.system_prompt
        )))
    }
}

/// Discovers and loads user skills from a directory
pub struct SkillLoader {
    skills_dir: PathBuf,
}

impl SkillLoader {
    /// Create a new skill loader
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    /// Discover and load all user skills
    pub async fn load_skills(&self) -> Result<Vec<Box<dyn Skill>>> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            debug!("Skills directory does not exist: {:?}", self.skills_dir);
            return Ok(skills);
        }

        let mut entries = tokio::fs::read_dir(&self.skills_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_md_path = path.join("skill.md");
            if !skill_md_path.exists() {
                debug!("Skipping {:?}: no skill.md found", path);
                continue;
            }

            match self.load_skill_from_path(&skill_md_path).await {
                Ok(skill) => {
                    info!("Loaded user skill: {}", skill.info().name);
                    skills.push(skill);
                }
                Err(e) => {
                    warn!("Failed to load skill from {:?}: {}", path, e);
                }
            }
        }

        info!("Loaded {} user skills", skills.len());
        Ok(skills)
    }

    /// Load a single skill from its skill.md path
    async fn load_skill_from_path(&self, skill_md_path: &Path) -> Result<Box<dyn Skill>> {
        let content = tokio::fs::read_to_string(skill_md_path)
            .await
            .context("Failed to read skill.md")?;

        let manifest = SkillManifest::from_markdown(&content)?;
        let body = SkillManifest::get_body(&content)?.to_string();

        // Check for optional script
        let scripts_dir = skill_md_path.parent().unwrap().join("scripts");
        let script_path = if scripts_dir.exists() {
            // Look for main script
            let main_script = scripts_dir.join("main.sh")
                .exists()
                .then(|| scripts_dir.join("main.sh"))
                .or_else(|| scripts_dir.join("main.py").exists().then(|| scripts_dir.join("main.py")))
                .or_else(|| scripts_dir.join("main.js").exists().then(|| scripts_dir.join("main.js")));
            main_script
        } else {
            None
        };

        let skill = UserSkill::new(manifest, body, script_path);
        Ok(Box::new(skill))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parsing() {
        let content = r#"---
name: test-skill
description: A test skill
version: 1.0.0
author: Test Author
---

This is the skill body with instructions.
"#;

        let manifest = SkillManifest::from_markdown(content).unwrap();
        assert_eq!(manifest.name, "test-skill");
        assert_eq!(manifest.description, "A test skill");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.author, Some("Test Author".to_string()));

        let body = SkillManifest::get_body(content).unwrap();
        assert!(body.contains("This is the skill body"));
    }

    #[test]
    fn test_missing_frontmatter() {
        let content = "No frontmatter here";
        let result = SkillManifest::from_markdown(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_fields() {
        let content = r#"---
name: test-skill
---
body"#;

        let result = SkillManifest::from_markdown(content);
        assert!(result.is_err()); // description is required
    }
}
