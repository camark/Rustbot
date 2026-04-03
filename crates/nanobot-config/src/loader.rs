//! Configuration loader and error handling

use crate::schema::Config;
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),

    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Invalid config: {0}")]
    ValidationError(String),
}

/// Configuration loader
pub struct ConfigLoader {
    config_path: PathBuf,
}

impl ConfigLoader {
    /// Create a new config loader with the given path
    pub fn new(config_path: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
        }
    }

    /// Load configuration from the default location (~/.nanobot/config.json)
    pub fn from_default_location() -> Result<Self, ConfigError> {
        let config_dir = crate::paths::get_config_dir()
            .ok_or_else(|| ConfigError::NotFound("Home directory not found".into()))?;

        Ok(Self::new(config_dir.join("config.json")))
    }

    /// Load and parse the configuration file
    pub fn load(&self) -> Result<Config, ConfigError> {
        // Check if file exists
        if !self.config_path.exists() {
            return Err(ConfigError::NotFound(self.config_path.clone()));
        }

        // Read file content
        let content = fs::read_to_string(&self.config_path)?;

        // Parse JSON
        let mut config: Config = serde_json::from_str(&content)?;

        // Apply environment variable overrides
        self.apply_env_overrides(&mut config);

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize and write
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, content)?;

        Ok(())
    }

    /// Check if config file exists
    pub fn exists(&self) -> bool {
        self.config_path.exists()
    }

    /// Get the config file path
    pub fn path(&self) -> &Path {
        &self.config_path
    }

    /// Apply environment variable overrides
    ///
    /// Environment variables use NANOBOT_ prefix with double underscore for nesting:
    /// - NANOBOT_PROVIDERS__OPENROUTER__API_KEY=xxx
    /// - NANOBOT_AGENTS__DEFAULTS__MODEL=anthropic/claude-3-5-sonnet
    fn apply_env_overrides(&self, config: &mut Config) {
        // Helper to get env var and set field
        let get_env = |key: &str| env::var(key).ok();

        // Providers
        if let Some(key) = get_env("NANOBOT_PROVIDERS__OPENROUTER__API_KEY") {
            config.providers.openrouter.api_key = key;
        }
        if let Some(key) = get_env("NANOBOT_PROVIDERS__ANTHROPIC__API_KEY") {
            config.providers.anthropic.api_key = key;
        }
        if let Some(key) = get_env("NANOBOT_PROVIDERS__OPENAI__API_KEY") {
            config.providers.openai.api_key = key;
        }
        if let Some(key) = get_env("NANOBOT_PROVIDERS__DEEPSEEK__API_KEY") {
            config.providers.deepseek.api_key = key;
        }

        // Agent defaults
        if let Some(model) = get_env("NANOBOT_AGENTS__DEFAULTS__MODEL") {
            config.agents.defaults.model = model;
        }
        if let Some(provider) = get_env("NANOBOT_AGENTS__DEFAULTS__PROVIDER") {
            config.agents.defaults.provider = provider;
        }
        if let Some(workspace) = get_env("NANOBOT_AGENTS__DEFAULTS__WORKSPACE") {
            config.agents.defaults.workspace = workspace;
        }
    }

    /// Create a default configuration file
    pub fn create_default(&self) -> Result<Config, ConfigError> {
        let config = Config::default();
        self.save(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let loader = ConfigLoader::new(&config_path);

        // Create default config
        let config = loader.create_default().unwrap();

        // Verify defaults
        assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");
        assert_eq!(config.agents.defaults.provider, "auto");
        assert!(loader.exists());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");

        let loader = ConfigLoader::new(&config_path);
        let result = loader.load();

        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }
}
