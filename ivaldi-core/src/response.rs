//! # Response Types
//!
//! The unified response format for all ivaldi operations.
//!
//! ## PURPOSE
//!
//! Every operation in ivaldi returns an `IvaldiResponse<T>`. This wrapper ensures:
//! - Consistent structure for agents to parse
//! - Advisory channel always available
//! - Status explicitly stated (not inferred from absence)
//!
//! ## THE THIRD CHANNEL
//!
//! Traditional Unix uses stdout for data and stderr for errors.
//! ivaldi adds a third channel: **advisory** - coaching messages that aren't errors.
//!
//! ```text
//! Channel 1 (stdout)  → result field     → The actual data
//! Channel 2 (stderr)  → error field      → Hard failures
//! Channel 3 (stdinfo) → advisory field   → Coaching, hints, wisdom
//! ```
//!
//! ## AGENT EVALUATION HEURISTIC
//!
//! ```text
//! if status != "success":
//!     evaluate_fully()          # Hard failure
//! elif advisory.len() > 0:
//!     scan_for_warnings()       # Brief review
//! else:
//!     continue_workflow()       # Clean success, minimal thinking
//! ```
//!
//! ## DESIGN DECISIONS
//!
//! 1. **Status is explicit** - Never infer from null checks
//! 2. **Advisory is always an array** - Even if empty, for consistent parsing
//! 3. **Error includes machine-readable code** - For pattern matching
//! 4. **Result is generic** - Operations define their own success payload

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::advisory::AdvisoryMessage;

/// The status of an ivaldi operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    /// Operation succeeded completely
    Success,
    /// Operation succeeded but with warnings
    Warning,
    /// Operation failed
    Error,
}

/// Unified response for all ivaldi operations.
///
/// Every tool call returns this structure, ensuring agents can
/// parse responses consistently regardless of the operation type.
///
/// # Example
///
/// ```json
/// {
///   "status": "success",
///   "result": { "path": "/src/main.rs", "bytes_written": 1234 },
///   "advisory": [
///     { "source": "tool", "level": "info", "message": "Backup created" }
///   ],
///   "error": null
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IvaldiResponse<T> {
    /// Operation status: success, warning, or error
    pub status: ResponseStatus,
    
    /// The operation result (null on error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    
    /// Advisory messages - the third channel
    /// Always an array for consistent parsing
    #[serde(default)]
    pub advisory: Vec<AdvisoryMessage>,
    
    /// Error details (null on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDetail>,
}

/// Structured error information.
///
/// Machine-readable error codes enable pattern matching:
/// - ADT can learn which errors are common
/// - Agents can decide retry vs abort
/// - Logs can aggregate by error code
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorDetail {
    /// Machine-readable error code (e.g., "file_not_found", "permission_denied")
    pub code: String,
    
    /// Human/agent readable error message
    pub message: String,
    
    /// Suggested action to resolve the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    
    /// Additional context for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

// ============================================================================
// BUILDER METHODS
// ============================================================================

impl<T> IvaldiResponse<T> {
    /// Create a successful response with result
    pub fn success(result: T) -> Self {
        Self {
            status: ResponseStatus::Success,
            result: Some(result),
            advisory: Vec::new(),
            error: None,
        }
    }
    
    /// Create a successful response with result and multiple advisories
    pub fn success_with_advisory(result: T, advisories: Vec<AdvisoryMessage>) -> Self {
        Self {
            status: ResponseStatus::Success,
            result: Some(result),
            advisory: advisories,
            error: None,
        }
    }
    
    /// Create an error response
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: ResponseStatus::Error,
            result: None,
            advisory: Vec::new(),
            error: Some(ErrorDetail {
                code: code.into(),
                message: message.into(),
                hint: None,
                context: None,
            }),
        }
    }
    
    /// Create an error response from an IvaldiError
    pub fn from_error(err: crate::error::IvaldiError) -> Self {
        Self {
            status: ResponseStatus::Error,
            result: None,
            advisory: Vec::new(),
            error: Some(ErrorDetail {
                code: err.code().to_string(),
                message: err.to_string(),
                hint: None,
                context: None,
            }),
        }
    }

    /// Add an advisory message
    pub fn with_advisory(mut self, advisory: AdvisoryMessage) -> Self {
        self.advisory.push(advisory);
        self
    }
    
    /// Add multiple advisory messages
    pub fn with_advisories(mut self, mut advisories: Vec<AdvisoryMessage>) -> Self {
        self.advisory.append(&mut advisories);
        self
    }
    
    /// Add a hint to an error response
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        if let Some(ref mut err) = self.error {
            err.hint = Some(hint.into());
        }
        self
    }
    
    /// Check if the response represents a successful operation
    pub fn is_success(&self) -> bool {
        matches!(self.status, ResponseStatus::Success) && self.result.is_some()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_success_response() {
        let resp: IvaldiResponse<String> = IvaldiResponse::success("hello".to_string());
        assert_eq!(resp.status, ResponseStatus::Success);
        assert!(resp.error.is_none());
    }
    
    #[test]
    fn test_error_response() {
        let resp: IvaldiResponse<()> = IvaldiResponse::error("file_not_found", "Path does not exist");
        assert_eq!(resp.status, ResponseStatus::Error);
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
    }
}