//! Context builder for agent prompts

use chrono::{DateTime, Utc};
use std::path::Path;

/// Runtime context tag
const RUNTIME_CONTEXT_TAG: &str = "=== CURRENT TIME AND CONTEXT ===";

/// Context builder for constructing LLM messages
pub struct ContextBuilder {
    workspace_dir: std::path::PathBuf,
    timezone: String,
}

impl ContextBuilder {
    /// Create a new context builder
    pub fn new(workspace_dir: impl AsRef<Path>, timezone: impl Into<String>) -> Self {
        Self {
            workspace_dir: workspace_dir.as_ref().to_path_buf(),
            timezone: timezone.into(),
        }
    }

    /// Get the timezone
    pub fn timezone(&self) -> &str {
        &self.timezone
    }

    /// Build runtime context message
    pub fn build_runtime_context(&self) -> String {
        let now: DateTime<Utc> = Utc::now();
        let time_str = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();

        format!(
            "{}\nCurrent time: {}\nTimezone: {}\nWorking directory: {}\n==================",
            RUNTIME_CONTEXT_TAG,
            time_str,
            self.timezone,
            self.workspace_dir.display(),
        )
    }

    /// Build messages for LLM
    pub fn build_messages(
        &self,
        history: Vec<serde_json::Value>,
        current_message: &str,
        channel: &str,
        chat_id: &str,
    ) -> Vec<serde_json::Value> {
        let mut messages = Vec::with_capacity(history.len() + 3);

        // System message with runtime context
        let runtime_context = self.build_runtime_context();
        messages.push(serde_json::json!({
            "role": "system",
            "content": self.build_system_prompt(),
        }));

        // Add runtime context as user message
        messages.push(serde_json::json!({
            "role": "user",
            "content": runtime_context,
        }));

        // Add conversation history
        for msg in history {
            messages.push(msg);
        }

        // Add current message
        messages.push(serde_json::json!({
            "role": "user",
            "content": current_message,
        }));

        messages
    }

    /// Build the system prompt
    fn build_system_prompt(&self) -> String {
        r#"You are RustBot, an AI assistant running in a Rust environment.

Your capabilities:
- Answer questions and have conversations
- Execute shell commands (when available)
- Read and write files
- Search and fetch web content
- Use tools provided to you

Guidelines:
- Be helpful, harmless, and honest
- Admit when you don't know something
- Use tools when they can help answer questions
- Be concise but thorough
- Show your reasoning for complex problems

You are currently running in CLI mode. Respond naturally to user messages."#.to_string()
    }

    /// Build messages with media support
    pub fn build_messages_with_media(
        &self,
        history: Vec<serde_json::Value>,
        current_message: &str,
        media: &[String],
        channel: &str,
        chat_id: &str,
    ) -> Vec<serde_json::Value> {
        let mut messages = Vec::with_capacity(history.len() + 3);

        // System message
        messages.push(serde_json::json!({
            "role": "system",
            "content": self.build_system_prompt(),
        }));

        // Runtime context
        let runtime_context = self.build_runtime_context();
        messages.push(serde_json::json!({
            "role": "user",
            "content": runtime_context,
        }));

        // History
        for msg in history {
            messages.push(msg);
        }

        // Current message with optional media
        if media.is_empty() {
            messages.push(serde_json::json!({
                "role": "user",
                "content": current_message,
            }));
        } else {
            // Build multimodal content
            let mut content = Vec::new();

            // Add text content
            content.push(serde_json::json!({
                "type": "text",
                "text": current_message,
            }));

            // Add media (assuming images for now)
            for media_url in media {
                content.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": media_url,
                    },
                }));
            }

            messages.push(serde_json::json!({
                "role": "user",
                "content": content,
            }));
        }

        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_runtime_context() {
        let temp_dir = TempDir::new().unwrap();
        let builder = ContextBuilder::new(temp_dir.path(), "UTC");

        let context = builder.build_runtime_context();

        assert!(context.contains(RUNTIME_CONTEXT_TAG));
        assert!(context.contains("UTC"));
        assert!(context.contains("Working directory:"));
    }

    #[test]
    fn test_build_messages() {
        let temp_dir = TempDir::new().unwrap();
        let builder = ContextBuilder::new(temp_dir.path(), "UTC");

        let history = vec![
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Hi there!"}),
        ];

        let messages = builder.build_messages(history, "How are you?", "cli", "direct");

        assert!(messages.len() >= 4); // system + context + history + current
        assert_eq!(messages[0]["role"], "system");
    }
}
