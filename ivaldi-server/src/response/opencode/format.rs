//! OpenCode-specific response formatting
//!
//! This module handles responses for OpenCode's MCP client, which expects
//! a hybrid format compatible with the Vercel AI SDK validation requirements.
//!
//! OpenCode uses @ai-sdk/openai-compatible which has strict validation:
//! - Responses must have either `choices` (success) OR `error` (failure)
//! - Never both, never neither
//! - Schema flattening is required for oneOf/allOf/anyOf compatibility

use crate::response::ResponseFormatter;
use chrono;
use ivaldi_core::IvaldiResponse;
use serde_json::{json, Value};

/// OpenCode response formatter for handling OpenCode-specific requirements
pub struct OpenCodeFormatter;

impl ResponseFormatter for OpenCodeFormatter {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn format_success(&self, response: IvaldiResponse<Value>) -> Result<Value, String> {
        // OpenCode uses Vercel AI SDK which expects full OpenAI Chat Completions format
        let content = response
            .content
            .unwrap_or_else(|| Value::String("".to_string()));
        let content_str = match content {
            Value::String(s) => s,
            Value::Null => "".to_string(),
            _ => content.to_string(), // Convert other types to string
        };

        // Create full OpenAI Chat Completions response structure
        let timestamp = chrono::Utc::now().timestamp();
        let result = json!({
            "id": format!("ivaldi-{}", timestamp),
            "object": "chat.completion",
            "created": timestamp,
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
        tracing::debug!(
            "OpenCode formatter created response with keys: {:?}",
            result.as_object().unwrap().keys().collect::<Vec<_>>()
        );
        Ok(result)
    }

    fn format_error(&self, response: IvaldiResponse<Value>) -> Result<Value, String> {
        // OpenCode error format - compatible with OpenAI style
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

    fn detect_compatibility(&self, request: &Value) -> bool {
        // Detect OpenCode client through multiple indicators

        // 1. Check for OpenCode-specific metadata in request
        if let Some(meta) = request.get("_meta") {
            if let Some(client_type) = meta.get("client_type").and_then(|v| v.as_str()) {
                if client_type.to_lowercase().contains("opencode") {
                    return true;
                }
            }
            if let Some(client_name) = meta.get("client_name").and_then(|v| v.as_str()) {
                if client_name.to_lowercase().contains("opencode") {
                    return true;
                }
            }
        }

        // 2. Check for OpenCode-specific user agent patterns
        if let Some(params) = request.get("params") {
            if let Some(user_agent) = params.get("user_agent").and_then(|v| v.as_str()) {
                if user_agent.to_lowercase().contains("opencode") {
                    return true;
                }
            }
        }

        // 3. Check for OpenCode-specific initialization options
        if let Some(params) = request.get("params") {
            if let Some(init_opts) = params.get("initializationOptions") {
                if let Some(client_info) = init_opts.get("clientInfo") {
                    if let Some(name) = client_info.get("name").and_then(|v| v.as_str()) {
                        if name.to_lowercase().contains("opencode") {
                            return true;
                        }
                    }
                }
            }
        }

        // 4. Environment-based detection (as fallback, though we know env vars aren't passed)
        if std::env::var("OPENAI_API_KEY").is_ok() {
            // If OPENAI_API_KEY is present, likely an OpenAI-compatible client like OpenCode
            return true;
        }

        // 5. Check parent process (if available)
        if let Ok(ppid) = std::env::var("PPID") {
            // This is a heuristic - in practice we'd need to check the actual process name
            // For now, just log this for debugging
            tracing::trace!("Parent process ID: {}", ppid);
        }

        false
    }
}
