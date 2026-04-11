//! Streaming utilities for CLI

use std::io;
use tokio::io::{AsyncWriteExt, Stdout};

/// Handle streaming output
#[allow(dead_code)]
pub struct StreamWriter {
    stdout: Stdout,
    buffer: String,
}

#[allow(dead_code)]
impl StreamWriter {
    pub fn new() -> Self {
        Self {
            stdout: tokio::io::stdout(),
            buffer: String::new(),
        }
    }

    /// Write a delta to stdout
    pub async fn write_delta(&mut self, delta: &str) -> io::Result<()> {
        self.buffer.push_str(delta);
        self.stdout.write_all(delta.as_bytes()).await?;
        let _ = self.stdout.flush().await;
        Ok(())
    }

    /// End the stream
    pub async fn end(&mut self) -> io::Result<()> {
        self.stdout.write_all(b"\n").await?;
        let _ = self.stdout.flush().await;
        self.buffer.clear();
        Ok(())
    }

    /// Get the full content
    pub fn content(&self) -> &str {
        &self.buffer
    }
}

impl Default for StreamWriter {
    fn default() -> Self {
        Self::new()
    }
}
