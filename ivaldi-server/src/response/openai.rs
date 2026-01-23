//! OpenAI API response formatting
//!
//! This module handles responses in OpenAI chat completions format
//! for clients that expect this format (like OpenCode).

use chrono;
use ivaldi_core::IvaldiResponse;
use serde_json::{json, Value};

/// Format response in OpenAI chat completions API standard
/// This sends actual OpenAI API format that OpenCode expects
/// - Success: {"choices": [{"message": {"content": "..."}}], ...}
/// - Error: {"error": {"message": "..."}}
pub fn handle_openai_response(response: IvaldiResponse<Value>) -> Result<Value, String> {
    if response.is_error {
        // Error response: Only error object, no choices
        let result = json!({
            "error": {
                "message": response.error.as_ref().unwrap().message,
                "type": "tool_error"
            }
        });
        Ok(result)
    } else {
        // Success response: OpenAI chat completion format
        let content = response
            .content
            .unwrap_or_else(|| Value::String("".to_string()));
        let content_str = match content {
            Value::String(s) => s,
            Value::Null => "".to_string(),
            _ => content.to_string(), // Convert other types to string
        };

        let result = json!({
            "id": format!("ivaldi-{}", chrono::Utc::now().timestamp()),
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": "ivaldi-mcp",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content_str
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0
            }
        });
        Ok(result)
    }
}
