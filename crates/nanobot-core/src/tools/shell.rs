//! Shell execution tool

use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::tools::{Tool, ToolError, ToolResult};

/// Shell execution tool configuration
#[derive(Debug, Clone)]
pub struct ShellToolConfig {
    pub enable: bool,
    pub timeout: u64,
    pub restrict_to_workspace: bool,
    pub path_append: String,
}

impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            enable: true,
            timeout: 60,
            restrict_to_workspace: false,
            path_append: String::new(),
        }
    }
}

/// Shell execution tool
pub struct ShellTool {
    working_dir: String,
    config: ShellToolConfig,
}

impl ShellTool {
    /// Create a new shell tool
    pub fn new(working_dir: impl Into<String>, config: ShellToolConfig) -> Self {
        Self {
            working_dir: working_dir.into(),
            config,
        }
    }

    /// Execute a command with timeout
    async fn execute_command(&self, cmd: &str) -> ToolResult<String> {
        let timeout = Duration::from_secs(self.config.timeout);

        // Determine shell based on platform
        let (shell, shell_arg) = if cfg!(windows) {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        debug!("Executing command: {} {}", shell, cmd);

        let mut command = Command::new(shell);
        command.arg(shell_arg).arg(cmd);
        command.current_dir(&self.working_dir);
        command.kill_on_drop(true);

        // Append to PATH if configured
        if !self.config.path_append.is_empty() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!(
                "{};{}",
                self.config.path_append,
                current_path
            );
            command.env("PATH", new_path);
        }

        // Execute with timeout
        let output = tokio::time::timeout(timeout, command.output())
            .await
            .map_err(|_| ToolError::Timeout)?
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("{}{}", stdout, stderr))
        } else {
            // Return stderr as part of result for debugging
            Ok(format!(
                "Command failed with code {:?}:\n{}\n{}",
                output.status.code(),
                stdout,
                stderr
            ))
        }
    }

    /// Check if command is safe to execute
    fn is_safe_command(&self, cmd: &str) -> bool {
        // Block dangerous commands
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "del /f /s /q c:\\*",
            "format",
            "mkfs",
            "dd if=/dev/zero",
            ":(){:|:&};:",
            "shutdown -r",
            "reboot",
            "init 0",
            "pkill -9",
        ];

        let cmd_lower = cmd.to_lowercase();
        for pattern in dangerous_patterns {
            if cmd_lower.contains(pattern) {
                warn!("Blocked dangerous command: {}", cmd);
                return false;
            }
        }

        true
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "exec"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Use with caution. Returns command output."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute",
                },
            },
            "required": ["command"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        if !self.config.enable {
            return Err(ToolError::Execution(
                "Shell execution is disabled".to_string(),
            ));
        }

        let command = params
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'command' parameter".to_string()))?;

        // Safety check
        if !self.is_safe_command(command) {
            return Err(ToolError::Execution(
                "Command blocked for safety".to_string(),
            ));
        }

        info!("Executing shell command: {}", command);

        let output = self.execute_command(command).await?;

        Ok(json!({
            "output": output,
            "truncated": output.len() > 10000,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands() {
        let tool = ShellTool::new(".", ShellToolConfig::default());

        assert!(tool.is_safe_command("ls -la"));
        assert!(tool.is_safe_command("pwd"));
        assert!(tool.is_safe_command("echo hello"));
    }

    #[test]
    fn test_dangerous_commands() {
        let tool = ShellTool::new(".", ShellToolConfig::default());

        assert!(!tool.is_safe_command("rm -rf /"));
        assert!(!tool.is_safe_command("format c:"));
    }
}
