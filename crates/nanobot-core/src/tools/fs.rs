//! Filesystem tools for file and directory operations

use async_trait::async_trait;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use tracing::{debug, info, warn};

use crate::tools::{Tool, ToolError, ToolResult};

/// Resolve a user directory path in a cross-platform way.
/// Handles both Chinese and English directory names on Linux/Unix systems
/// by reading XDG user directories configuration.
fn resolve_user_dir(dir_name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Determine the XDG variable name based on directory name
    let xdg_var = match dir_name {
        "桌面" | "Desktop" => "XDG_DESKTOP_DIR",
        "文档" | "Documents" => "XDG_DOCUMENTS_DIR",
        "下载" | "Downloads" => "XDG_DOWNLOAD_DIR",
        "模板" | "Templates" => "XDG_TEMPLATES_DIR",
        "公共" | "Public" => "XDG_PUBLICSHARE_DIR",
        "音乐" | "Music" => "XDG_MUSIC_DIR",
        "图片" | "Pictures" => "XDG_PICTURES_DIR",
        "视频" | "Videos" => "XDG_VIDEOS_DIR",
        _ => return None,
    };

    // Try XDG environment variable first
    if let Ok(xdg_path) = env::var(xdg_var) {
        let xdg_path = PathBuf::from(xdg_path);
        if xdg_path.exists() {
            return Some(xdg_path);
        }
    }

    // Fallback: read from ~/.config/user-dirs.dirs
    let config_path = home.join(".config/user-dirs.dirs");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let prefix = format!("{}=", xdg_var);
            for line in content.lines() {
                if let Some(value) = line.strip_prefix(&prefix) {
                    // Parse "$HOME/xxx" or "/absolute/path"
                    let path = value.trim_matches('"').trim();
                    if let Some(relative) = path.strip_prefix("$HOME/") {
                        return Some(home.join(relative));
                    } else if let Some(absolute) = path.strip_prefix('$') {
                        // Handle ${HOME} format
                        if let Some(rel) = absolute.strip_prefix("{HOME}/") {
                            return Some(home.join(rel));
                        }
                    } else if path.starts_with('/') {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }
    }

    // Final fallback: use English names in home directory
    let fallback = match dir_name {
        "桌面" | "Desktop" => "Desktop",
        "文档" | "Documents" => "Documents",
        "下载" | "Downloads" => "Downloads",
        "模板" | "Templates" => "Templates",
        "公共" | "Public" => "Public",
        "音乐" | "Music" => "Music",
        "图片" | "Pictures" => "Pictures",
        "视频" | "Videos" => "Videos",
        _ => return None,
    };

    Some(home.join(fallback))
}

// ============================================================================
// Read File Tool
// ============================================================================

/// Read file content tool
pub struct ReadFileTool {
    workspace: PathBuf,
    allowed_dir: Option<PathBuf>,
}

impl ReadFileTool {
    pub fn new(workspace: impl AsRef<Path>, allowed_dir: Option<PathBuf>) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            allowed_dir,
        }
    }

    fn resolve_path(&self, path: &str) -> ToolResult<PathBuf> {
        // Expand ~ to home directory
        let path = if path.starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?;
            // Check if it's a known user directory (Desktop, Documents, etc.)
            let dir_name = &path[2..]; // Remove ~/ prefix
            let resolved = if let Some(user_dir) = resolve_user_dir(dir_name) {
                user_dir
            } else {
                // Not a known user directory, just join with home
                home.join(dir_name)
            };
            resolved
        } else if path == "~" {
            dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?
        } else {
            PathBuf::from(path)
        };

        // Make absolute if relative
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        // Normalize path (remove .. and . components without following symlinks)
        // This avoids issues with canonicalize() on Windows returning UNC paths
        let mut normalized = PathBuf::new();
        for component in abs_path.components() {
            match component {
                Component::ParentDir => {
                    // Go up one directory if possible
                    normalized.pop();
                }
                Component::CurDir => {
                    // Skip current directory
                }
                Component::Normal(_) | Component::Prefix(_) | Component::RootDir => {
                    normalized.push(component);
                }
            }
        }

        // Now canonicalize to verify file exists and resolve symlinks
        let canonical = normalized
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("File not found: {} ({})", e, normalized.display())))?;

        // Check if within allowed directory (compare canonical paths)
        if let Some(ref allowed) = self.allowed_dir {
            if !canonical.starts_with(allowed) {
                return Err(ToolError::Execution(
                    "Access denied: path traversal detected".to_string(),
                ));
            }
        }

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the content of a file. Returns the file content as text."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (relative to workspace or absolute)",
                },
            },
            "required": ["path"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        let path = params
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path' parameter".to_string()))?;

        info!("Reading file: {}", path);

        let file_path = self.resolve_path(path)?;

        let content = tokio::fs::read_to_string(&file_path)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to read file: {}", e)))?;

        Ok(json!({
            "content": content,
            "path": path,
            "size": content.len(),
        }))
    }
}

// ============================================================================
// Write File Tool
// ============================================================================

/// Write file content tool
pub struct WriteFileTool {
    workspace: PathBuf,
    allowed_dir: Option<PathBuf>,
}

impl WriteFileTool {
    pub fn new(workspace: impl AsRef<Path>, allowed_dir: Option<PathBuf>) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            allowed_dir,
        }
    }

    fn resolve_path(&self, path: &str) -> ToolResult<PathBuf> {
        // Expand ~ to home directory
        let path = if path.starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?;
            // Check if it's a known user directory (Desktop, Documents, etc.)
            let dir_name = &path[2..]; // Remove ~/ prefix
            let resolved = if let Some(user_dir) = resolve_user_dir(dir_name) {
                user_dir
            } else {
                // Not a known user directory, just join with home
                home.join(dir_name)
            };
            resolved
        } else if path == "~" {
            dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?
        } else {
            PathBuf::from(path)
        };

        // Make absolute if relative
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        Ok(abs_path.clone())
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write (relative to workspace or absolute)",
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file",
                },
            },
            "required": ["path", "content"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        let path = params
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path' parameter".to_string()))?;

        let content = params
            .get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'content' parameter".to_string()))?;

        info!("Writing file: {} ({} bytes)", path, content.len());

        let file_path = self.resolve_path(path)?;

        // Canonicalize parent path for security check (create parent if needed)
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ToolError::Execution(format!("Failed to create directories: {}", e)))?;
            // After creating parent, canonicalize to verify it's within allowed dir
            if let Some(ref allowed) = self.allowed_dir {
                let canonical_parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
                if !canonical_parent.starts_with(allowed) {
                    return Err(ToolError::Execution(
                        "Access denied: path traversal detected".to_string(),
                    ));
                }
            }
        }

        tokio::fs::write(&file_path, content)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to write file: {}", e)))?;

        Ok(json!({
            "success": true,
            "path": path,
            "bytes_written": content.len(),
        }))
    }
}

// ============================================================================
// Edit File Tool
// ============================================================================

/// Edit file content tool (search and replace)
pub struct EditFileTool {
    workspace: PathBuf,
    allowed_dir: Option<PathBuf>,
}

impl EditFileTool {
    pub fn new(workspace: impl AsRef<Path>, allowed_dir: Option<PathBuf>) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            allowed_dir,
        }
    }

    fn resolve_path(&self, path: &str) -> ToolResult<PathBuf> {
        // First, handle special Chinese path names (Linux/Unix)
        let path_str = match path {
            "桌面" | "桌面/" => "~/桌面",
            "文档" | "文档/" => "~/文档",
            "下载" | "下载/" => "~/下载",
            // Windows English equivalents
            "Desktop" | "Desktop/" => "~/Desktop",
            "Documents" | "Documents/" => "~/Documents",
            "Downloads" | "Downloads/" => "~/Downloads",
            _ => path,
        };

        // Expand ~ to home directory
        let path = if path_str.starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?;
            let rel_path = path_str[2..].trim_start_matches('/');
            home.join(rel_path)
        } else if path_str == "~" {
            dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?
        } else {
            PathBuf::from(path_str)
        };

        // Make absolute if relative
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        // Normalize path (remove .. and . components)
        use std::path::Component;
        let mut normalized = PathBuf::new();
        for component in abs_path.components() {
            match component {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::CurDir => {
                    // Skip
                }
                Component::Normal(_) | Component::Prefix(_) | Component::RootDir => {
                    normalized.push(component);
                }
            }
        }

        // Canonicalize to verify file exists
        let canonical = normalized
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("File not found: {} ({})", e, normalized.display())))?;

        // Check if within allowed directory
        if let Some(ref allowed) = self.allowed_dir {
            if !canonical.starts_with(allowed) {
                return Err(ToolError::Execution(
                    "Access denied: path traversal detected".to_string(),
                ));
            }
        }

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by searching for specific content and replacing it."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit",
                },
                "search": {
                    "type": "string",
                    "description": "Text to search for",
                },
                "replace": {
                    "type": "string",
                    "description": "Text to replace with",
                },
                "all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false, only first)",
                },
            },
            "required": ["path", "search", "replace"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        let path = params
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path' parameter".to_string()))?;

        let search = params
            .get("search")
            .and_then(|s| s.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'search' parameter".to_string()))?;

        let replace = params
            .get("replace")
            .and_then(|r| r.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'replace' parameter".to_string()))?;

        let replace_all = params
            .get("all")
            .and_then(|a| a.as_bool())
            .unwrap_or(false);

        info!("Editing file: {} (search: '{}')", path, search);

        let file_path = self.resolve_path(path)?;

        let content = tokio::fs::read_to_string(&file_path)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to read file: {}", e)))?;

        let occurrence = if replace_all {
            content.matches(search).count()
        } else {
            content.find(search).map(|_| 1).unwrap_or(0)
        };

        if occurrence == 0 {
            return Err(ToolError::Execution(
                "Search text not found in file".to_string(),
            ));
        }

        let new_content = if replace_all {
            content.replace(search, replace)
        } else {
            let mut parts = content.splitn(2, search);
            let result = format!("{}{}{}", parts.next().unwrap(), replace, parts.next().unwrap_or(""));
            result
        };

        tokio::fs::write(&file_path, &new_content)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to write file: {}", e)))?;

        Ok(json!({
            "success": true,
            "path": path,
            "occurrences_replaced": occurrence,
        }))
    }
}

// ============================================================================
// List Directory Tool
// ============================================================================

/// List directory contents tool
pub struct ListDirTool {
    workspace: PathBuf,
    allowed_dir: Option<PathBuf>,
}

impl ListDirTool {
    pub fn new(workspace: impl AsRef<Path>, allowed_dir: Option<PathBuf>) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            allowed_dir,
        }
    }

    fn resolve_path(&self, path: &str) -> ToolResult<PathBuf> {
        // Expand ~ to home directory
        let path = if path.starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?;
            // Check if it's a known user directory (Desktop, Documents, etc.)
            let dir_name = &path[2..]; // Remove ~/ prefix
            let resolved = if let Some(user_dir) = resolve_user_dir(dir_name) {
                user_dir
            } else {
                // Not a known user directory, just join with home
                home.join(dir_name)
            };
            resolved
        } else if path == "~" {
            dirs::home_dir()
                .ok_or_else(|| ToolError::Execution("Home directory not found".to_string()))?
        } else {
            PathBuf::from(path)
        };

        // Make absolute if relative
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        // Normalize path (remove .. and . components)
        use std::path::Component;
        let mut normalized = PathBuf::new();
        for component in abs_path.components() {
            match component {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::CurDir => {
                    // Skip
                }
                Component::Normal(_) | Component::Prefix(_) | Component::RootDir => {
                    normalized.push(component);
                }
            }
        }

        // Canonicalize to verify directory exists
        let canonical = normalized
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("Directory not found: {} ({})", e, normalized.display())))?;

        // Check if within allowed directory
        if let Some(ref allowed) = self.allowed_dir {
            if !canonical.starts_with(allowed) {
                return Err(ToolError::Execution(
                    "Access denied: path traversal detected".to_string(),
                ));
            }
        }

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the contents of a directory. Returns files and subdirectories with 'name', 'type', and 'full_path' fields. Use the 'full_path' value to read files with read_file tool. Use the 'pattern' parameter to filter files (e.g., '*.pdf' for PDF files, '*.txt' for text files)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list (e.g., '~/Desktop' or '~/Documents')",
                },
                "pattern": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., '*.pdf' for PDF files, '*.txt' for text files)",
                },
            },
            "required": ["path"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        let path = params
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path' parameter".to_string()))?;

        let pattern = params
            .get("pattern")
            .and_then(|p| p.as_str());

        info!("Listing directory: {} (pattern: {:?})", path, pattern);

        let dir_path = self.resolve_path(path)?;

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&dir_path)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to read entry: {}", e)))?
        {
            let file_name = entry
                .file_name()
                .to_string_lossy()
                .to_string();

            let file_type = entry
                .file_type()
                .await
                .map(|t| {
                    if t.is_dir() {
                        "directory"
                    } else if t.is_file() {
                        "file"
                    } else {
                        "other"
                    }
                })
                .unwrap_or("unknown");

            // Apply pattern filter if specified
            if let Some(pat) = pattern {
                if !match_pattern(&file_name, pat) {
                    continue;
                }
            }

            entries.push(json!({
                "name": file_name,
                "type": file_type,
                "full_path": dir_path.join(&file_name).to_string_lossy(),
            }));
        }

        Ok(json!({
            "path": path,
            "entries": entries,
            "count": entries.len(),
        }))
    }
}

/// Simple glob pattern matching (supports only * wildcard)
fn match_pattern(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }

    // Handle *.ext pattern
    if pattern.starts_with("*.") {
        let ext = &pattern[1..]; // Get ".ext"
        return name.ends_with(ext);
    }

    // Handle * pattern (match all)
    if pattern == "*" {
        return true;
    }

    // Exact match
    name == pattern
}
