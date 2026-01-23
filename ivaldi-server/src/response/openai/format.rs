//! OpenAI Chat Completions Response Formatting
//!
//! This module implements pure OpenAI Chat Completions API formatting.
//! It produces responses compatible with the OpenAI API specification.

use ivaldi_core::IvaldiResponse;
use serde_json::{json, Value};

/// Format a successful OpenAI response
///
/// Creates a chat completion response in OpenAI API format.
/// The content is serialized to a string for the message content field.
pub fn format_success_response(response: IvaldiResponse<Value>) -> Result<Value, String> {
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

/// Format an OpenAI error response
///
/// Creates an error response in OpenAI API style format.
pub fn format_error_response(response: IvaldiResponse<Value>) -> Result<Value, String> {
    let error_message = response
        .error
        .as_ref()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "Unknown error".to_string());

    let result = json!({
        "error": {
            "message": error_message,
            "type": "tool_error"
        }
    });
    Ok(result)
}
