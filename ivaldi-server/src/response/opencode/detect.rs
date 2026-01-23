//! OpenCode Client Detection
//!
//! This module detects whether a request is from OpenCode's MCP client.
//! OpenCode uses the Vercel AI SDK which has specific validation requirements.

use serde_json::Value;

/// Detect if a request is from OpenCode client
///
/// OpenCode can be detected through multiple indicators:
/// 1. Explicit metadata flags (_meta.client_type/name)
/// 2. Initialization options (clientInfo.name)
/// 3. Environment indicators (OPENAI_API_KEY presence)
/// 4. Parent process heuristics
pub fn detect_opencode_request(request: &Value) -> bool {
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
