//! Memory management for context window control

use tiktoken_rs::{cl100k_base, CoreBPE};

/// Count tokens in a text string using cl100k_base encoding (used by GPT-4, Claude, etc.)
pub fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().expect("Failed to get tokenizer");
    bpe.encode_ordinary(text).len()
}

/// Count tokens for a message (role + content)
pub fn count_message_tokens(role: &str, content: &str) -> usize {
    // OpenAI format: 4 tokens per message + role tokens + content tokens
    let bpe = cl100k_base().expect("Failed to get tokenizer");
    4 + bpe.encode_ordinary(role).len() + bpe.encode_ordinary(content).len()
}

/// Memory manager for context window control
pub struct MemoryManager {
    max_tokens: u32,
    reserved_for_response: u32,
}

impl MemoryManager {
    pub fn new(max_tokens: u32, reserved_for_response: u32) -> Self {
        Self {
            max_tokens,
            reserved_for_response,
        }
    }

    /// Get the effective token limit for input messages
    pub fn input_token_limit(&self) -> u32 {
        self.max_tokens.saturating_sub(self.reserved_for_response)
    }

    /// Truncate messages to fit within token limit
    /// Returns the truncated messages and the number of tokens used
    pub fn truncate_messages(
        &self,
        messages: &[serde_json::Value],
    ) -> (Vec<serde_json::Value>, usize) {
        let limit = self.input_token_limit() as usize;

        // Start from the most recent messages and work backwards
        let mut selected: Vec<&serde_json::Value> = Vec::new();
        let mut total_tokens = 0;

        // Always keep system messages from the beginning
        let mut system_messages: Vec<&serde_json::Value> = Vec::new();
        for msg in messages {
            if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                if role == "system" {
                    if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                        system_messages.push(msg);
                        total_tokens += count_message_tokens("system", content);
                    }
                }
            }
        }

        // Add recent messages until we hit the limit
        for msg in messages.iter().rev() {
            // Skip system messages (already counted)
            if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                if role == "system" {
                    continue;
                }

                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    let msg_tokens = count_message_tokens(role, content);
                    if total_tokens + msg_tokens > limit {
                        break;
                    }
                    total_tokens += msg_tokens;
                    selected.push(msg);
                } else if let Some(tool_calls) = msg.get("tool_calls") {
                    // Tool call message
                    let content_str = tool_calls.to_string();
                    let msg_tokens = count_message_tokens(role, &content_str);
                    if total_tokens + msg_tokens > limit {
                        break;
                    }
                    total_tokens += msg_tokens;
                    selected.push(msg);
                }
            }
        }

        // Build result: system messages first, then selected recent messages (in original order)
        let mut result: Vec<serde_json::Value> = system_messages.into_iter().cloned().collect();

        // Add selected messages in reverse order (to restore original time order)
        for msg in selected.into_iter().rev() {
            result.push(msg.clone());
        }

        (result, total_tokens)
    }

    /// Check if consolidation is needed based on message count and token usage
    pub fn needs_consolidation(&self, messages: &[serde_json::Value], threshold_count: usize) -> bool {
        if messages.len() > threshold_count {
            return true;
        }

        let (_, token_count) = self.truncate_messages(messages);
        token_count > (self.input_token_limit() as usize * 80 / 100) // 80% of limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        let text = "Hello, world!";
        let tokens = count_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < 100); // Sanity check
    }

    #[test]
    fn test_memory_manager_truncate() {
        let manager = MemoryManager::new(1000, 200);

        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are a helpful assistant."}),
            serde_json::json!({"role": "user", "content": "Hello!"}),
            serde_json::json!({"role": "assistant", "content": "Hi! How can I help you?"}),
        ];

        let (truncated, tokens) = manager.truncate_messages(&messages);
        assert_eq!(truncated.len(), 3);
        assert!(tokens > 0);
    }

    #[test]
    fn test_memory_manager_limit() {
        let manager = MemoryManager::new(100, 20); // Very small limit

        let messages: Vec<serde_json::Value> = (0..50)
            .map(|i| serde_json::json!({"role": "user", "content": format!("Message {}", i)}))
            .collect();

        let (truncated, _tokens) = manager.truncate_messages(&messages);
        // Should truncate to fit within limit
        assert!(truncated.len() < messages.len());
    }
}
