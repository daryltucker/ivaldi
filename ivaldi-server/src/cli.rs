//! CLI argument definitions for ivaldi-server
//!
//! This module defines the command-line interface for the server.
//! The Args struct is used both at runtime (for parsing) and at build-time
//! (for auto-generating documentation via clap's CommandFactory).

use clap::{Parser, ValueEnum};
use ivaldi_core::session::ConversationMode;

#[derive(Debug, Clone, ValueEnum)]
pub enum ResponseMode {
    /// MCP Standard: errors in `error` field
    Mcp,
    /// OpenAI chat completions API format (for OpenCode compatibility)
    #[clap(name = "openai")]
    Openai,
    /// Auto-detect based on client capabilities (default)
    Auto,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Transport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
pub enum SandboxFeature {
    /// Filesystem isolation (Read-only root, bind-mounted project)
    Fs,
    /// Network isolation (No internet access)
    Net,
    /// Enable all isolation features
    All,
}

/// ivaldi-server: MCP server for AI Agent file operations
#[derive(Parser, Debug, Clone)]
#[command(name = "ivaldi-server")]
#[command(about = "MCP server for AI Agent file operations", long_about = None)]
pub struct Args {
    /// Conversation ID for naked/stdio drivers (overrides IDE metadata)
    #[arg(long, env = "IVALDI_CONVERSATION_ID")]
    pub conversation_id: Option<String>,

    /// Conversation mode: persist (default, full tracking) or incognito (ephemeral, no vecdb)
    #[arg(long, env = "IVALDI_CONVERSATION_MODE", value_parser = parse_conversation_mode)]
    pub conversation_mode: Option<ConversationMode>,

    /// API Key for authenticated services
    #[arg(long, env = "IVALDI_API_KEY")]
    pub api_key: Option<String>,

    /// Tool namespace prefix (helps avoid clashes with other MCP servers)
    #[arg(long, env = "IVALDI_TOOL_NAMESPACE")]
    pub tool_namespace: Option<String>,

    /// Path to a custom configuration file
    #[arg(long, short = 'c', env = "IVALDI_CONFIG")]
    pub config: Option<String>,

    /// Execution sandboxing features (comma-separated)
    /// Example: --exec-sandboxing=fs,net
    #[arg(long, value_delimiter = ',', env = "IVALDI_EXEC_SANDBOXING")]
    pub exec_sandboxing: Option<Vec<SandboxFeature>>,

    /// Transport mode: stdio (default) or http
    #[arg(long, value_enum, default_value_t = Transport::Stdio, env = "IVALDI_TRANSPORT")]
    pub transport: Transport,

    /// Response format mode: mcp, openai, or auto (default: auto)
    #[arg(long, value_enum, default_value_t = ResponseMode::Auto, env = "IVALDI_RESPONSE_MODE")]
    pub response_mode: ResponseMode,

    /// Port for HTTP server (default: 8080)
    #[arg(long, default_value_t = 8080, env = "IVALDI_PORT")]
    pub port: u16,
}

fn parse_conversation_mode(s: &str) -> Result<ConversationMode, String> {
    s.parse()
}
