//! Response Types
//!
//! Shared types used across all response formatters.
//! This module ensures consistency in response structures.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Common fields for OpenAI-style chat completion responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: UsageInfo,
}

/// A single choice in a chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: i32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

/// A message in a chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// OpenAI-style error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiErrorResponse {
    pub error: OpenAiError,
}

/// Error details in OpenAI format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
}

/// MCP JSON-RPC success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSuccessResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

/// MCP JSON-RPC error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpErrorResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub error: McpError,
}

/// Error details in MCP JSON-RPC format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

// ============================================================================
// BUILDER HELPERS
// ============================================================================

impl ChatCompletionResponse {
    /// Create a new chat completion response with the given content
    pub fn new(content: String) -> Self {
        Self {
            id: format!("ivaldi-{}", chrono::Utc::now().timestamp()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: "ivaldi-mcp".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: "tool_calls".to_string(),
            }],
            usage: UsageInfo {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        }
    }
}

impl OpenAiErrorResponse {
    /// Create a new error response with the given message and type
    pub fn new(message: String, error_type: String) -> Self {
        Self {
            error: OpenAiError {
                message,
                error_type,
            },
        }
    }

    /// Create a tool error response
    pub fn tool_error(message: String) -> Self {
        Self::new(message, "tool_error".to_string())
    }
}

impl McpSuccessResponse {
    /// Create a new MCP success response
    pub fn new(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result,
        }
    }
}

impl McpErrorResponse {
    /// Create a new MCP error response
    pub fn new(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            error: McpError {
                code,
                message,
                data: None,
            },
        }
    }

    /// Create a method not found error
    pub fn method_not_found(id: Value, message: String) -> Self {
        Self::new(id, -32601, message)
    }

    /// Create a parse error
    pub fn parse_error(id: Value, message: String) -> Self {
        Self::new(id, -32700, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_completion_response() {
        let response = ChatCompletionResponse::new("Hello world".to_string());
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.model, "ivaldi-mcp");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.content, "Hello world");
    }

    #[test]
    fn test_openai_error_response() {
        let error = OpenAiErrorResponse::tool_error("Test error".to_string());
        assert_eq!(error.error.message, "Test error");
        assert_eq!(error.error.error_type, "tool_error");
    }

    #[test]
    fn test_mcp_success_response() {
        let response =
            McpSuccessResponse::new(serde_json::json!(1), serde_json::json!({"status": "ok"}));
        assert_eq!(response.jsonrpc, "2.0");
    }

    #[test]
    fn test_mcp_error_response() {
        let error = McpErrorResponse::method_not_found(
            serde_json::json!(1),
            "Method not found".to_string(),
        );
        assert_eq!(error.error.code, -32601);
    }
}
