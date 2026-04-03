//! Filesystem tools for file and directory operations

use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::tools::{Tool, ToolError, ToolResult};

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
        let path = PathBuf::from(path);

        // Make absolute if relative
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        // Check if within allowed directory
        if let Some(ref allowed) = self.allowed_dir {
            if !abs_path.starts_with(allowed) {
                return Err(ToolError::Execution(format!(
                    "Access denied: {} is outside allowed directory",
                    path.display()
                )));
            }
        }

        // Security: resolve and verify no traversal
        let canonical = abs_path
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("File not found: {}", e)))?;

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
        let path = PathBuf::from(path);

        // Make absolute if relative
        let abs_path = if path.is_absolute() {
            path
        } else {
            self.workspace.join(&path)
        };

        // Check if within allowed directory
        if let Some(ref allowed) = self.allowed_dir {
            if !abs_path.starts_with(allowed) {
                return Err(ToolError::Execution(format!(
                    "Access denied: {} is outside allowed directory",
                    abs_path.display()
                )));
            }
        }

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

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ToolError::Execution(format!("Failed to create directories: {}", e)))?;
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
        let path = PathBuf::from(path);
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        if let Some(ref allowed) = self.allowed_dir {
            if !abs_path.starts_with(allowed) {
                return Err(ToolError::Execution(format!(
                    "Access denied: {} is outside allowed directory",
                    path.display()
                )));
            }
        }

        let canonical = abs_path
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("File not found: {}", e)))?;

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
        let path = PathBuf::from(path);
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            self.workspace.join(&path)
        };

        if let Some(ref allowed) = self.allowed_dir {
            if !abs_path.starts_with(allowed) {
                return Err(ToolError::Execution(format!(
                    "Access denied: {} is outside allowed directory",
                    path.display()
                )));
            }
        }

        let canonical = abs_path
            .canonicalize()
            .map_err(|e| ToolError::Execution(format!("Directory not found: {}", e)))?;

        Ok(canonical)
    }
}

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the contents of a directory. Returns files and subdirectories."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list",
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

        info!("Listing directory: {}", path);

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

            entries.push(json!({
                "name": file_name,
                "type": file_type,
            }));
        }

        Ok(json!({
            "path": path,
            "entries": entries,
            "count": entries.len(),
        }))
    }
}
