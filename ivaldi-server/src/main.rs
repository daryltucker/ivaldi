//! # ivaldi-server
//!
//! MCP (Model Context Protocol) server for AI Agent file operations.
//!
//! ## PURPOSE
//!
//! This is the primary interface for AI Agents. While humans may use
//! the CLI (`ivaldi`), agents should connect to this MCP server.
//!
//! ## MCP TOOLS (Planned)
//!
//! ### File Operations
//! - `find_files` - Find files matching pattern (.aiignore aware)
//! - `list_dir` - List directory with metadata
//! - `read_file` - Read file with optional line range
//! - `create_file` - Create new file (with pre-flight)
//! - `write_file` - Overwrite file (with backup)
//! - `edit_file` - AST-based editing
//!
//! ### CLI Proxy
//! - `run_command` - Execute whitelisted commands
//!
//! ### Recovery
//! - `undo` - Revert last operation(s)
//! - `history` - View operation journal
//!
//! ## THE THIRD CHANNEL
//!
//! Every MCP response uses `IvaldiResponse<T>` which includes:
//! - `status` - success/warning/error
//! - `result` - the operation result
//! - `advisory` - coaching messages (stdinfo)
//! - `error` - structured error details
//!
//! ## ADT INTEGRATION
//!
//! Before each operation, the server may query the ADT:
//! ```text
//! vecdb search "error: {similar_operation}" --collection adt_wisdom
//! ```
//! Matches are injected into the advisory channel as suggestions.
//!
//! ## TRANSPORT
//!
//! - Default: stdio (for integration with editors/agents)
//! - Optional: HTTP (for remote/multi-agent scenarios)
//!
//! ## PHILOSOPHY
//!
//! The server is the "frontal cortex" for agent operations:
//! 1. Receive tool call
//! 2. Query ADT for wisdom (prophetic errors)
//! 3. Run pre-flight validation
//! 4. Execute via ivaldi-core
//! 5. Collect advisories
//! 6. Return structured response

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::{json, Value};
use tracing::{info, error, warn};
use tracing_subscriber::prelude::*;
use std::panic;

mod tools;
mod adt;
mod state;
mod protocol;
mod server_http;

use state::ServerState;
use ivaldi_server::{Args, Transport};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments (includes ENV var fallback)
    let cli_args = Args::parse();
    
    // Initialize logging with IVALDI_LOG (user-friendly) or RUST_LOG (advanced)
    let log_filter = if let Ok(ivaldi_log) = std::env::var("IVALDI_LOG") {
        // User-friendly IVALDI_LOG levels
        match ivaldi_log.to_lowercase().as_str() {
            "off" => "off".to_string(),
            "error" => "ivaldi_server=error,ivaldi_core=error".to_string(),
            "warn" => "ivaldi_server=warn,ivaldi_core=warn".to_string(),
            "info" => "ivaldi_server=info,ivaldi_core=info".to_string(),
            "debug" => "ivaldi_server=debug,ivaldi_core=info".to_string(),
            "trace" => "ivaldi_server=trace,ivaldi_core=debug".to_string(),
            _ => {
                eprintln!("Warning: Invalid IVALDI_LOG value '{}'. Valid values: off, error, warn, info, debug, trace. Defaulting to 'info'.", ivaldi_log);
                "ivaldi_server=info,ivaldi_core=info".to_string()
            }
        }
    } else if std::env::var("RUST_LOG").is_ok() {
        // Fall back to RUST_LOG for advanced users
        std::env::var("RUST_LOG").unwrap()
    } else {
        // Default to info level
        "ivaldi_server=info,ivaldi_core=info".to_string()
    };
    
    let journald_layer = tracing_journald::layer().ok();
    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    
    tracing_subscriber::registry()
        .with(journald_layer)
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::new(log_filter))
        .init();
    
    // Global panic hook: log panic info before process terminates
    // This ensures Agent gets useful info even on catastrophic failure
    panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        
        let location = panic_info.location().map(|loc| {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        }).unwrap_or_else(|| "unknown location".to_string());
        
        // Log to journald/stderr so it's captured
        eprintln!("PANIC at {}: {}", location, msg);
        // Also try tracing (may not work if runtime is dead)
        tracing::error!(location = %location, message = %msg, "CRITICAL: Server panic");
    }));
    
    info!(version = env!("CARGO_PKG_VERSION"), "ivaldi-server starting");
    info!("Status: Operational (Phase 4: Session Management)");
    
    // 1. Initialize State (Config, Sessions, Registry)
    let state = match ServerState::new() {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "CRITICAL: Failed to initialize server state");
            std::process::exit(1);
        }
    };
    
    if state.config().enable_gitignore {
        info!("Config: Gitignore Filtering ENABLED");
    }

    if let Some(config) = &cli_args.config {
        info!(path = config, "Config: custom file loaded");
    }

    if let Some(key) = &cli_args.api_key {
        let masked = if key.len() > 8 {
            format!("{}...{}", &key[0..4], &key[key.len()-4..])
        } else {
            "********".to_string()
        };
        info!(api_key = masked, "Config: API Key present");
    }

    // Initialize Middleware (ADT support enabled via IVALDI_ADT_ENABLED)
    let adt_client = if std::env::var("IVALDI_ADT_ENABLED").is_ok() {
        let url = std::env::var("IVALDI_VECDB_URL").ok();
        info!("ADT: Wisdom Layer ENABLED (Target: {})", url.as_deref().unwrap_or("http://localhost:8080"));
        Some(adt::AdtClient::new(url))
    } else {
        None
    };

    let middleware = std::sync::Arc::new(tools::middleware::Middleware::new(adt_client));

    // 2. Transport Loop
    match cli_args.transport {
        Transport::Stdio => {
            info!("Mode: MCP JSON-RPC via stdio (Async)");
            let stdin = tokio::io::stdin();
            let reader = BufReader::new(stdin);
            let mut lines = reader.lines();
            let mut stdout = tokio::io::stdout();
            
            let id_re = regex::Regex::new(r#""id"\s*:\s*(\d+|"[^"]+")"#).unwrap();
            while let Some(line) = lines.next_line().await? {
                if line.trim().is_empty() { continue; }

                let request: Value = match serde_json::from_str(&line) {
                    Ok(req) => req,
                    Err(e) => {
                        // Log parse failure with context for ADT learning
                        warn!(
                            error = %e,
                            input_length = line.len(),
                            input_preview = %if line.len() > 100 { &line[..100] } else { &line },
                            "JSON parse failed - possible Agent formatting error"
                        );
                        // Try to extract ID for error response (best effort)
                        // Optimization: compile regex once, outside loop, or use static.
                        // For now, moving it up or just cleaning logic.
                        if let Some(captures) = id_re.captures(&line) && let Some(id_match) = captures.get(1) {
                                let id_str = id_match.as_str();
                                let error_response = json!({
                                    "jsonrpc": "2.0",
                                    "id": serde_json::from_str::<serde_json::Value>(id_str).unwrap_or(json!(null)),
                                    "error": {
                                        "code": -32700,
                                        "message": "Parse error",
                                        "data": format!("Invalid JSON: {}", e)
                                    }
                                });
                                let mut resp_str = serde_json::to_string(&error_response).unwrap();
                                resp_str.push('\n');
                                let _ = stdout.write_all(resp_str.as_bytes()).await;
                                let _ = stdout.flush().await;
                        }
                        continue;
                    },
                };

                let id = request.get("id").unwrap_or(&Value::Null).clone();
                let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
                let middleware = middleware.clone(); // Clone ARC
                let state_clone = state.clone();
                let cli_args_clone = cli_args.clone();

                tokio::select! {
                    result = async {
                        match method {
                            "initialize" => protocol::handle_initialize(&request, &state_clone),
                            "notifications/initialized" => { Ok(json!({})) }, // Empty success
                            "tools/list" => protocol::handle_tools_list(),
                            "tools/call" => protocol::handle_tools_call(&request, &state_clone, &cli_args_clone, &middleware).await,
                            _ => {
                                if id.is_null() { return Ok(json!({})); }
                                Err("Method not found".to_string())
                            }
                        }
                    } => {
                        match result {
                            Ok(res) => {
                                if !id.is_null() && method != "notifications/initialized" {
                                    let response = json!({ "jsonrpc": "2.0", "id": id, "result": res });
                                    let mut response_string = serde_json::to_string(&response).unwrap();
                                    response_string.push('\n');
                                    if let Err(e) = stdout.write_all(response_string.as_bytes()).await {
                                        error!(error = %e, "Failed to write to stdout");
                                        break;
                                    }
                                    let _ = stdout.flush().await;
                                }
                            },
                            Err(err) => {
                                if !id.is_null() {
                                    let response = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": err } });
                                    let mut response_string = serde_json::to_string(&response).unwrap();
                                    response_string.push('\n');
                                    let _ = stdout.write_all(response_string.as_bytes()).await;
                                    let _ = stdout.flush().await;
                                }
                            }
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received SIGINT, shutting down stdio loop...");
                        break;
                    }
                }
            }
        },
        Transport::Http => {
            info!(port = cli_args.port, "Mode: MCP JSON-RPC via HTTP");
            server_http::run(cli_args.port, state, middleware, cli_args.clone()).await?;
        }
    }
    
    Ok(())
}