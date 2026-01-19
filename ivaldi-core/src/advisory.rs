//! # Advisory Messages
//!
//! The third channel: coaching messages that aren't errors.
//!
//! ## PURPOSE
//!
//! Advisory messages provide context, warnings, and wisdom without causing failure.
//! Like Dark Souls messages from players (Tool) and developers (Server/ADT).
//!
//! ## MESSAGE SOURCES
//!
//! | Source | Origin | Example |
//! |--------|--------|---------|
//! | Tool | The operation itself | "File has trailing whitespace" |
//! | Server | The MCP server | "Collection needs optimization" |
//! | ADT | Collective wisdom | "This pattern failed 3x before" |
//!
//! ## ADVISORY LEVELS
//!
//! | Level | Meaning | Agent Action |
//! |-------|---------|--------------|
//! | Info | FYI, no action needed | Log and continue |
//! | Warn | Something unusual | Brief review |
//! | Suggest | Recommended action | Consider following |
//!
//! ## EMBEDDING SUPPORT
//!
//! Advisory messages can include embeddings from vecdb searches:
//! - ADT queries for similar past failures
//! - Documentation search for relevant hints
//! - Error code lookup for known solutions
//!
//! This enables "prophetic error correction" - warning agents before they fail.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Source of an advisory message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AdvisorySource {
    /// Message from the tool/operation itself
    Tool,
    /// Message from the MCP server infrastructure
    Server,
    /// Message from the Abstract Decision Tree (collective wisdom)
    Adt,
}

/// Severity level of an advisory message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AdvisoryLevel {
    /// Informational - no action needed
    Info,
    /// Warning - something unusual, brief review recommended
    Warn,
    /// Suggestion - recommended action to consider
    Suggest,
}

/// An advisory message in the third channel.
///
/// Advisory messages provide coaching without failure. They enable:
/// - Tool observations ("I noticed X")
/// - Server notes ("System state is Y")
/// - ADT wisdom ("Previous similar attempt failed")
///
/// # Example
///
/// ```json
/// {
///   "source": "adt",
///   "level": "suggest",
///   "message": "Similar edit failed yesterday due to borrow checker",
///   "action": "Consider adding lifetime parameter",
///   "embedding": [0.123, 0.456, ...]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdvisoryMessage {
    /// Who generated this message
    pub source: AdvisorySource,
    
    /// How important is this message
    pub level: AdvisoryLevel,
    
    /// The advisory content (structured data)
    pub content: serde_json::Value,
    
    /// Optional suggested action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    
    /// Optional embedding from vecdb search
    /// Used for ADT queries on error patterns
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    
    /// Optional reference to related documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

// ============================================================================
// BUILDER METHODS
// ============================================================================

impl AdvisoryMessage {
    /// Create an info-level advisory from the tool
    pub fn tool_info<T: Serialize>(content: T) -> Self {
        Self {
            source: AdvisorySource::Tool,
            level: AdvisoryLevel::Info,
            content: serde_json::to_value(content).unwrap_or(serde_json::Value::Null),
            action: None,
            embedding: None,
            reference: None,
        }
    }
    
    /// Create a warning from the tool
    pub fn tool_warn<T: Serialize>(content: T) -> Self {
        Self {
            source: AdvisorySource::Tool,
            level: AdvisoryLevel::Warn,
            content: serde_json::to_value(content).unwrap_or(serde_json::Value::Null),
            action: None,
            embedding: None,
            reference: None,
        }
    }
    
    /// Create a suggestion from the ADT (collective wisdom)
    pub fn adt_suggest<T: Serialize>(content: T, action: impl Into<String>) -> Self {
        Self {
            source: AdvisorySource::Adt,
            level: AdvisoryLevel::Suggest,
            content: serde_json::to_value(content).unwrap_or(serde_json::Value::Null),
            action: Some(action.into()),
            embedding: None,
            reference: None,
        }
    }
    
    /// Add an embedding from vecdb search
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }
    
    /// Add a documentation reference
    pub fn with_reference(mut self, reference: impl Into<String>) -> Self {
        self.reference = Some(reference.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_info() {
        let msg = AdvisoryMessage::tool_info("File has trailing whitespace");
        assert_eq!(msg.source, AdvisorySource::Tool);
        assert_eq!(msg.level, AdvisoryLevel::Info);
        assert_eq!(msg.content, serde_json::Value::String("File has trailing whitespace".to_string()));
    }
    
    #[test]
    fn test_adt_suggest_with_action() {
        let msg = AdvisoryMessage::adt_suggest(
            "This pattern failed before",
            "Consider using Arc<Mutex<T>>"
        );
        assert_eq!(msg.source, AdvisorySource::Adt);
        assert!(msg.action.is_some());
    }
}
