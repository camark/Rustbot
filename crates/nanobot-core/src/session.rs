//! Session management for conversation history

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{fs, io};

/// Session configuration
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Maximum number of messages to keep in memory (0 = unlimited)
    pub max_messages: usize,
    /// Maximum age in days before session is considered expired (0 = never expire)
    pub max_age_days: u32,
    /// Number of messages after which to trigger consolidation
    pub consolidate_threshold: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_messages: 100,
            max_age_days: 30,
            consolidate_threshold: 50,
        }
    }
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session key (e.g., "telegram:123456789")
    pub key: String,

    /// Channel name
    pub channel: String,

    /// Chat identifier
    pub chat_id: String,

    /// Message history
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,

    /// Created timestamp
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session
    pub fn new(key: impl Into<String>, channel: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            channel: channel.into(),
            chat_id: chat_id.into(),
            messages: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, message: serde_json::Value) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Get recent messages
    pub fn get_history(&self, max_messages: usize) -> Vec<serde_json::Value> {
        if max_messages == 0 {
            self.messages.clone()
        } else {
            self.messages
                .iter()
                .rev()
                .take(max_messages)
                .cloned()
                .collect()
        }
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Clear old messages (for memory consolidation)
    pub fn consolidate(&mut self, summary: String) {
        // Keep only the last few messages plus summary
        let keep_count = 4;
        let recent: Vec<_> = self.messages.iter().rev().take(keep_count).cloned().collect();

        let mut new_messages = Vec::with_capacity(1 + recent.len());

        // Add summary as system message
        new_messages.push(serde_json::json!({
            "role": "system",
            "content": format!("Previous conversation summary:\n{}", summary),
        }));

        // Add recent messages in original order
        for msg in recent.into_iter().rev() {
            new_messages.push(msg);
        }

        self.messages = new_messages;
        self.updated_at = Utc::now();
    }
}

/// Session manager
pub struct SessionManager {
    workspace_dir: PathBuf,
    sessions: parking_lot::Mutex<HashMap<String, Session>>,
}

use std::collections::HashMap;

impl SessionManager {
    /// Create a new session manager
    pub fn new(workspace_dir: impl AsRef<Path>) -> io::Result<Self> {
        let workspace_dir = workspace_dir.as_ref().to_path_buf();
        let sessions_dir = workspace_dir.join("sessions");

        // Create sessions directory
        fs::create_dir_all(&sessions_dir)?;

        Ok(Self {
            workspace_dir,
            sessions: parking_lot::Mutex::new(HashMap::new()),
        })
    }

    /// Get or create a session
    pub fn get_or_create(&self, key: impl Into<String>) -> SessionHandle {
        let key = key.into();

        // Try to get from cache
        {
            let sessions = self.sessions.lock();
            if let Some(session) = sessions.get(&key) {
                let mut session = session.clone();
                return SessionHandle::new(session, self.clone(), key);
            }
        }

        // Try to load from disk
        if let Some(session) = self.load_session(&key) {
            {
                let mut sessions = self.sessions.lock();
                sessions.insert(key.clone(), session.clone());
            } // Lock dropped here
            return SessionHandle::new(session, self.clone(), key);
        }

        // Create new session
        let (channel, chat_id) = key
            .split_once(':')
            .map(|(c, id)| (c.to_string(), id.to_string()))
            .unwrap_or_else(|| ("cli".to_string(), key.clone()));

        let session = Session::new(&key, channel, chat_id);

        {
            let mut sessions = self.sessions.lock();
            sessions.insert(key.clone(), session.clone());
        }

        SessionHandle::new(session, self.clone(), key)
    }

    /// Save a session to disk
    pub fn save(&self, session: &Session) -> io::Result<()> {
        let sessions_dir = self.workspace_dir.join("sessions");
        let safe_key = session.key.replace(['/', '\\', ':'], "_");
        let path = sessions_dir.join(format!("{}.json", safe_key));

        let content = serde_json::to_string_pretty(session)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        fs::write(path, content)
    }

    /// Load a session from disk
    fn load_session(&self, key: &str) -> Option<Session> {
        let sessions_dir = self.workspace_dir.join("sessions");
        let safe_key = key.replace(['/', '\\', ':'], "_");
        let path = sessions_dir.join(format!("{}.json", safe_key));

        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Get all session keys
    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.lock().keys().cloned().collect()
    }

    /// Remove a session
    pub fn remove(&self, key: &str) -> Option<Session> {
        let mut sessions = self.sessions.lock();
        let session = sessions.remove(key);

        // Also remove from disk
        if let Some(_) = &session {
            let sessions_dir = self.workspace_dir.join("sessions");
            let safe_key = key.replace(['/', '\\', ':'], "_");
            let path = sessions_dir.join(format!("{}.json", safe_key));
            let _ = fs::remove_file(path);
        }

        session
    }

    /// Clean up expired sessions (older than max_age_days)
    pub fn cleanup_expired(&self, max_age_days: u32) -> io::Result<usize> {
        let cutoff = Utc::now() - Duration::days(max_age_days as i64);
        let mut removed_count = 0;

        let sessions_to_remove: Vec<String> = {
            let sessions = self.sessions.lock();
            sessions
                .iter()
                .filter(|(_, s)| s.updated_at < cutoff)
                .map(|(k, _)| k.clone())
                .collect()
        };

        for key in sessions_to_remove {
            self.remove(&key);
            removed_count += 1;
        }

        // Also scan disk for old sessions not in memory
        let sessions_dir = self.workspace_dir.join("sessions");
        if sessions_dir.exists() {
            if let Ok(entries) = fs::read_dir(&sessions_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(session) = serde_json::from_str::<Session>(&content) {
                                if session.updated_at < cutoff {
                                    let _ = fs::remove_file(&path);
                                    removed_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }

    /// Consolidate old sessions by generating summaries
    pub fn consolidate_old_sessions(
        &self,
        threshold: usize,
        summary_generator: &dyn Fn(&Session) -> Option<String>,
    ) -> usize {
        let mut consolidated_count = 0;

        let sessions_to_consolidate: Vec<String> = {
            let sessions = self.sessions.lock();
            sessions
                .iter()
                .filter(|(_, s)| s.messages.len() > threshold)
                .map(|(k, _)| k.clone())
                .collect()
        };

        for key in sessions_to_consolidate {
            if let Some(session) = self.load_session(&key) {
                if let Some(summary) = summary_generator(&session) {
                    let mut sessions = self.sessions.lock();
                    if let Some(s) = sessions.get_mut(&key) {
                        s.consolidate(summary);
                        let _ = self.save(s);
                        consolidated_count += 1;
                    }
                }
            }
        }

        consolidated_count
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            workspace_dir: self.workspace_dir.clone(),
            sessions: parking_lot::Mutex::new(self.sessions.lock().clone()),
        }
    }
}

/// Handle to a session for modification
pub struct SessionHandle {
    session: Session,
    manager: SessionManager,
    key: String,
    dirty: bool,
}

impl SessionHandle {
    fn new(session: Session, manager: SessionManager, key: String) -> Self {
        Self {
            session,
            manager,
            key,
            dirty: false,
        }
    }

    /// Add a message
    pub fn add_message(&mut self, message: serde_json::Value) {
        self.session.add_message(message);
        self.dirty = true;
    }

    /// Get message history
    pub fn get_history(&self, max_messages: usize) -> Vec<serde_json::Value> {
        self.session.get_history(max_messages)
    }

    /// Get the session key
    pub fn key(&self) -> &str {
        &self.session.key
    }

    /// Save the session
    pub fn save(&mut self) -> io::Result<()> {
        self.manager.save(&self.session)?;
        self.dirty = false;
        Ok(())
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.save();
        }
    }
}
