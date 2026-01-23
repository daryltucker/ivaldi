//! Response Format Modules
//!
//! This module provides a modular response formatting system for the Ivaldi MCP server.
//! It supports multiple response formats to ensure compatibility with various clients:
//!
//! - **MCP**: Standard JSON-RPC format for proper MCP clients
//! - **OpenAI**: Chat completions API format for OpenAI-compatible clients
//! - **OpenCode**: Hybrid format for OpenCode's specific requirements
//!
//! ## Architecture
//!
//! ```text
//! response/
//! ├── mod.rs      # Registry + trait definitions
//! ├── types.rs    # Shared response types (ChatCompletion, McpResponse, etc.)
//! ├── mcp.rs      # MCP JSON-RPC formatter
//! ├── openai.rs   # OpenAI chat completions formatter
//! └── opencode.rs # OpenCode-specific formatter with auto-detection
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! let registry = response::get_registry();
//! let formatter = registry.get_formatter("opencode").unwrap();
//! let response = formatter.format_success(ivaldi_response)?;
//! ```

use ivaldi_core::IvaldiResponse;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================

pub mod mcp;
pub mod openai;
pub mod opencode;
pub mod types;

// ============================================================================
// RE-EXPORTS
// ============================================================================

// Re-export formatters for convenience
pub use mcp::{detect_mcp_request, format_error_content, format_success_content};
pub use openai::{detect_openai_request, format_error_response, format_success_response};
pub use opencode::{detect_opencode_request, OpenCodeFormatter};

// Re-export types for convenience
pub use types::{
    ChatChoice, ChatCompletionResponse, ChatMessage, McpError, McpErrorResponse,
    McpSuccessResponse, OpenAiError, OpenAiErrorResponse, UsageInfo,
};

// ============================================================================
// TRAIT DEFINITION
// ============================================================================

/// Core trait for response formatters.
///
/// Implement this trait to add support for new client response formats.
/// Each formatter is responsible for converting `IvaldiResponse` to the
/// specific format expected by the target client.
pub trait ResponseFormatter: Send + Sync {
    /// Unique name for this formatter (used in registry lookup)
    fn name(&self) -> &'static str;

    /// Format a successful tool response
    fn format_success(&self, response: IvaldiResponse<Value>) -> Result<Value, String>;

    /// Format an error response
    fn format_error(&self, response: IvaldiResponse<Value>) -> Result<Value, String>;

    /// Check if this formatter is compatible with the given request.
    /// Used for auto-detection of client types in `Auto` mode.
    fn detect_compatibility(&self, request: &Value) -> bool;
}

// ============================================================================
// REGISTRY
// ============================================================================

/// Registry for managing response formatters.
///
/// The registry maintains a collection of formatters that can be looked up
/// by name or auto-detected based on request characteristics.
pub struct ResponseRegistry {
    formatters: HashMap<String, Box<dyn ResponseFormatter>>,
}

impl ResponseRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            formatters: HashMap::new(),
        }
    }

    /// Register a new response formatter
    pub fn register<F: ResponseFormatter + 'static>(&mut self, formatter: F) {
        let name = formatter.name().to_string();
        self.formatters.insert(name, Box::new(formatter));
    }

    /// Get a formatter by name
    pub fn get_formatter(&self, name: &str) -> Option<&dyn ResponseFormatter> {
        self.formatters.get(name).map(|f| f.as_ref())
    }

    /// List all registered formatter names
    pub fn list_formatters(&self) -> Vec<&str> {
        self.formatters.keys().map(|s| s.as_str()).collect()
    }

    /// Auto-detect the best formatter for a request
    pub fn detect_formatter(&self, request: &Value) -> Option<&dyn ResponseFormatter> {
        for formatter in self.formatters.values() {
            if formatter.detect_compatibility(request) {
                return Some(formatter.as_ref());
            }
        }
        None
    }
}

impl Default for ResponseRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// GLOBAL REGISTRY
// ============================================================================

/// Global registry instance (thread-safe singleton)
static REGISTRY: OnceLock<ResponseRegistry> = OnceLock::new();

/// Get the global response registry (initialized with default formatters)
/// NOTE: MCP and OpenAI are now handled by dedicated modules, not registry
pub fn get_registry() -> &'static ResponseRegistry {
    REGISTRY.get_or_init(|| {
        let mut registry = ResponseRegistry::new();

        // Register built-in formatters (MCP and OpenAI handled separately)
        registry.register(opencode::OpenCodeFormatter);

        registry
    })
}
