//! OpenAI Client Detection
//!
//! This module detects whether a request is from an OpenAI-compatible client.
//! Detection is based on client metadata and environment indicators.

use serde_json::Value;

/// Detect if a request is from an OpenAI-compatible client
///
/// Checks for OpenAI-specific client hints and metadata.
/// This includes explicit client type declarations and environment indicators.
pub fn detect_openai_request(request: &Value) -> bool {
    // Check for OpenAI-style client hints
    if let Some(client_type) = request.get("_meta")
        .and_then(|meta| meta.get("client_type"))
        .and_then(|v| v.as_str()) {
        return client_type == "openai";
    }

    // Additional OpenAI detection could be added here
    // (e.g., specific headers, user agents, etc.)

    false
}
