//! Configuration file paths and directories

use std::path::{Path, PathBuf};

/// Get the base configuration directory (~/.nanobot)
pub fn get_config_dir() -> Option<PathBuf> {
    // On Windows, prefer USERPROFILE environment variable over dirs::home_dir()
    // to avoid issues with git-bash returning C:/ instead of C:/Users/<user>
    #[cfg(windows)]
    {
        // Always use USERPROFILE on Windows for consistency
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(userprofile).join(".nanobot"));
        }
        // Fallback to dirs::home_dir()
        dirs::home_dir().map(|home| home.join(".nanobot"))
    }

    #[cfg(not(windows))]
    {
        dirs::home_dir().map(|home| home.join(".nanobot"))
    }
}

/// Get the workspace directory (~/.nanobot/workspace)
pub fn get_workspace_dir() -> Option<PathBuf> {
    get_config_dir().map(|config| config.join("workspace"))
}

/// Configuration paths for a RustBot instance
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// Config file path (~/.nanobot/config.json)
    pub config_file: PathBuf,
    /// Workspace directory (~/.nanobot/workspace)
    pub workspace_dir: PathBuf,
    /// Cron directory (~/.nanobot/cron)
    pub cron_dir: PathBuf,
    /// Media directory (~/.nanobot/media)
    pub media_dir: PathBuf,
    /// Logs directory (~/.nanobot/logs)
    pub logs_dir: PathBuf,
}

impl ConfigPaths {
    /// Create config paths from base config directory
    pub fn from_config_dir(config_dir: &Path) -> Self {
        Self {
            config_file: config_dir.join("config.json"),
            workspace_dir: config_dir.join("workspace"),
            cron_dir: config_dir.join("cron"),
            media_dir: config_dir.join("media"),
            logs_dir: config_dir.join("logs"),
        }
    }

    /// Create default config paths
    pub fn default() -> Option<Self> {
        get_config_dir().map(|dir| Self::from_config_dir(&dir))
    }

    /// Ensure all directories exist
    pub fn create_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.workspace_dir)?;
        std::fs::create_dir_all(&self.cron_dir)?;
        std::fs::create_dir_all(&self.media_dir)?;
        std::fs::create_dir_all(&self.logs_dir)?;
        Ok(())
    }
}
