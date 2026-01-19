//! # Conversation Context
//!
//! ## PURPOSE
//! Provides conversation-level tracking within Sessions, enabling:
//! - Conversation-scoped ATT timelines
//! - Conversation-scoped ADT wisdom queries
//! - Ephemeral "Incognito" conversations for exploratory work
//!
//! ## KEY TYPES
//! - `ConversationMode` — Persist (full tracking) vs Incognito (ephemeral)
//! - `ConversationContext` — Conversation metadata (id, mode, timestamps)
//!
//! ## PHILOSOPHY
//! Conversations are chat-level contexts within a Session (project).
//! Sessions contain Conversations. Sessions are required, Conversations are optional enhancement.
//! When no conversation_id is provided, operations default to session-level context.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Conversation persistence mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConversationMode {
    /// Full vecdb tracking, survives restarts
    #[default]
    Persist,
    /// Memory only, no vecdb indexing
    Incognito,
}

impl std::fmt::Display for ConversationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Persist => write!(f, "persist"),
            Self::Incognito => write!(f, "incognito"),
        }
    }
}

impl std::str::FromStr for ConversationMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "persist" => Ok(Self::Persist),
            "incognito" => Ok(Self::Incognito),
            _ => Err(format!("Invalid conversation mode: '{}'. Expected 'persist' or 'incognito'", s)),
        }
    }
}

/// Conversation context within a Session
///
/// Represents a chat-level context (e.g., a single Antigravity conversation)
/// within a project-level Session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationContext {
    /// Unique conversation identifier (e.g., UUID from IDE)
    pub id: String,
    
    /// Persistence mode (Persist or Incognito)
    pub mode: ConversationMode,
    
    /// When this conversation started
    pub started: DateTime<Utc>,
    
    /// Last activity timestamp
    pub last_active: DateTime<Utc>,
}

impl ConversationContext {
    /// Create a new conversation in Persist mode (default)
    pub fn new(id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            mode: ConversationMode::default(),
            started: now,
            last_active: now,
        }
    }
    
    /// Create a new conversation in Incognito mode
    pub fn new_incognito(id: impl Into<String>) -> Self {
        let mut ctx = Self::new(id);
        ctx.mode = ConversationMode::Incognito;
        ctx
    }
    
    /// Update last_active timestamp to now
    pub fn touch(&mut self) {
        self.last_active = Utc::now();
    }
    
    /// Check if this conversation is in Persist mode
    pub fn is_persist(&self) -> bool {
        self.mode == ConversationMode::Persist
    }
    
    /// Check if this conversation is in Incognito mode
    pub fn is_incognito(&self) -> bool {
        self.mode == ConversationMode::Incognito
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_mode_default() {
        assert_eq!(ConversationMode::default(), ConversationMode::Persist);
    }

    #[test]
    fn test_conversation_mode_from_str() {
        assert_eq!("persist".parse::<ConversationMode>().unwrap(), ConversationMode::Persist);
        assert_eq!("incognito".parse::<ConversationMode>().unwrap(), ConversationMode::Incognito);
        assert_eq!("PERSIST".parse::<ConversationMode>().unwrap(), ConversationMode::Persist);
        assert!("invalid".parse::<ConversationMode>().is_err());
    }

    #[test]
    fn test_conversation_context_new() {
        let ctx = ConversationContext::new("test-123");
        assert_eq!(ctx.id, "test-123");
        assert_eq!(ctx.mode, ConversationMode::Persist);
        assert!(ctx.is_persist());
        assert!(!ctx.is_incognito());
    }

    #[test]
    fn test_conversation_context_new_incognito() {
        let ctx = ConversationContext::new_incognito("test-456");
        assert_eq!(ctx.id, "test-456");
        assert_eq!(ctx.mode, ConversationMode::Incognito);
        assert!(!ctx.is_persist());
        assert!(ctx.is_incognito());
    }

    #[test]
    fn test_conversation_context_touch() {
        let mut ctx = ConversationContext::new("test-789");
        let original_time = ctx.last_active;
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        ctx.touch();
        
        assert!(ctx.last_active > original_time);
    }
}
