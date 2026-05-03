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
//! - `find_files` - Find files matching pattern (.agentignore aware)
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
mod response;

use state::ServerState;
use ivaldi_server::{Args, Transport};
use ivaldi_core::config::GlobalConfig; // Added
use ivaldi_core::response::ErrorDetail;
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

    // Initialize tracing subscriber - journald is Linux-only
    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let journald_layer = tracing_journald::layer().ok();
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr);
        tracing_subscriber::registry()
            .with(journald_layer)
            .with(fmt_layer)
            .with(tracing_subscriber::EnvFilter::new(log_filter))
            .init();
    }

    #[cfg(not(target_os = "linux"))]
    {
        use std::io::IsTerminal;
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_ansi(std::io::stderr().is_terminal())
            .with_writer(std::io::stderr);
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(tracing_subscriber::EnvFilter::new(log_filter))
            .init();
    }

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

    // 0. Load Configuration
    // Try CLI path -> ENV -> Default
    let config_path = cli_args.config.as_deref().map(std::path::PathBuf::from);
    let mut config = GlobalConfig::load(config_path.as_deref()).unwrap_or_else(|e| {
        warn!(error = %e, "Failed to load config file. using defaults.");
        GlobalConfig::default()
    });

    // 1. Apply CLI Overrides (like exec_sandboxing)
    if let Some(features) = &cli_args.exec_sandboxing {
        use ivaldi_server::cli::SandboxFeature;
        use ivaldi_core::execution::IsolationMode;

        // If specific features are requested, we enable Bubblewrap mode
        if features.iter().any(|f| matches!(f, SandboxFeature::Fs | SandboxFeature::Net | SandboxFeature::All)) {
             config.safety.isolation_mode = IsolationMode::Bubblewrap;
        }

        for feature in features {
            match feature {
                SandboxFeature::Fs => config.safety.ro_bind_root = true,
                SandboxFeature::Net => config.safety.network_isolation = true,
                SandboxFeature::All => {
                    config.safety.isolation_mode = IsolationMode::Bubblewrap;
                    config.safety.ro_bind_root = true;
                    config.safety.network_isolation = true;
                }
            }
        }
    }

    info!("Status: Operational (v{}) - OpenCode Compatible", env!("CARGO_PKG_VERSION"));
    if let ivaldi_core::execution::IsolationMode::Bubblewrap = config.safety.isolation_mode {
        info!(
            fs_isolation = config.safety.ro_bind_root,
            network_isolation = config.safety.network_isolation,
            "Safety: Sandbox ENABLED"
        );
    }

    // 2. Initialize State with Config
    let state = match ServerState::new(config, cli_args.tool_namespace.clone(), cli_args.response_mode.clone()) {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "CRITICAL: Failed to initialize server state");
            std::process::exit(1);
        }
    };

    // Log the actual mode being used
    info!("🚀 Server initialized with response mode: {:?}", state.response_mode());
    info!("📝 Tool namespace: {:?}", state.tool_namespace());
    info!("🔧 Transport: {:?}", cli_args.transport);

    // Log environment variables for debugging (especially for OpenCode integration)
    info!("🌍 === Environment Variables Check ===");
    if let Ok(mode) = std::env::var("IVALDI_RESPONSE_MODE") {
        info!("🌍 ENV IVALDI_RESPONSE_MODE: {}", mode);
    } else {
        info!("🌍 ENV IVALDI_RESPONSE_MODE: not set (defaulting to AUTO mode)");
    }

    // Log all IVALDI_* environment variables for debugging
    for (key, value) in std::env::vars() {
        if key.starts_with("IVALDI_") {
            info!("🌍 ENV {}: {}", key, value);
        }
    }

    // Check for OpenCode-specific indicators
    if std::env::var("OPENAI_API_KEY").is_ok() {
        info!("🌍 Detected OPENAI_API_KEY - possible OpenCode client");
    }

    if let Ok(parent) = std::env::var("PARENT_PROCESS") {
        info!("🌍 Parent process: {}", parent);
    }

    info!("🌍 === End Environment Variables ===");


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

                 tracing::trace!("About to process method: '{}' with id: {:?}", method, id);
                 tokio::select! {
                     result = async {
                        match method {
                            "initialize" => protocol::handle_initialize(&request, &state_clone),
                            "notifications/initialized" => {
                                Ok(ivaldi_core::IvaldiResponse {
                                    content: Some(json!({})),
                                    is_error: false,
                                    ui_diffs: vec![],
                                    error: None,
                                    advisory: vec![],
                                })
                            },
                            "tools/list" => protocol::handle_tools_list(&state_clone),
                            "tools/call" => {
                                protocol::handle_tools_call(&request, &state_clone, &cli_args_clone, &middleware).await
                            },
                            _ => {
                                if id.is_null() {
                                    return Ok(ivaldi_core::IvaldiResponse {
                                        content: Some(json!({})),
                                        is_error: false,
                                        ui_diffs: vec![],
                                        error: None,
                                        advisory: vec![],
                                    });
                                }
                                Err("Method not found".to_string())
                            }
                        }
                     } => {
                         tracing::trace!("Method '{}' processed, result: {:?}", method, result.is_ok());
                         match result {
                              Ok(ivaldi_response) => {
                                 if !id.is_null() && method != "notifications/initialized" {
                                      // Format response based on mode using the formatter system
                                       let is_error = ivaldi_response.is_error;
                                      let current_mode = state_clone.response_mode();
                                      tracing::trace!("Response mode detected: {:?}", current_mode);
                                      let formatted_result = match *current_mode {
                                          ivaldi_server::cli::ResponseMode::Mcp => {
                                              // Use format_tool_response which handles both success/error and advisories
                                              Ok(response::mcp::format_tool_response(ivaldi_response))
                                          },
                                          ivaldi_server::cli::ResponseMode::Openai => {
                                              if is_error {
                                                  // Use new OpenAI module for error formatting
                                                  response::openai::format_error_response(ivaldi_response)
                                              } else {
                                                  // Use new OpenAI module for success formatting
                                                  response::openai::format_success_response(ivaldi_response)
                                              }
                                          },

                                          ivaldi_server::cli::ResponseMode::Auto => {
                                              // Try to detect the appropriate format
                                              if response::openai::detect_openai_request(&request) {
                                                  if is_error {
                                                      response::openai::format_error_response(ivaldi_response)
                                                  } else {
                                                      response::openai::format_success_response(ivaldi_response)
                                                  }
                                              } else {
                                                  // Fallback to MCP using new MCP module
                                                  Ok(response::mcp::format_tool_response(ivaldi_response))
                                              }
                                          }
                                     }.unwrap_or_else(|_| {
                                         json!({ "error": { "message": "Formatting error", "type": "formatting_error" } })
                                     });

                                     // ALL response modes use JSON-RPC envelope for stdio MCP transport
                                     // The response mode only affects what goes INSIDE the result field
                                     // DEFENSIVE: Bypass wrapping if the result is already a full JSON-RPC envelope
                                     let response_to_send = if formatted_result.get("jsonrpc").is_some() {
                                         formatted_result
                                     } else {
                                         json!({
                                             "jsonrpc": "2.0",
                                             "id": id,
                                             "result": formatted_result
                                         })
                                     };

                                    let mut response_string = serde_json::to_string(&response_to_send).unwrap();
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
                                    // Format error response based on mode
                                    let error_response = match *state_clone.response_mode() {
                                        ivaldi_server::cli::ResponseMode::Openai => {
                                            // OpenAI mode: format error using direct OpenAI formatting
                                            let error_detail = ErrorDetail {
                                                code: "tool_error".to_string(),
                                                message: err.clone(),
                                                hint: None,
                                                context: None,
                                            };
                                            let ivaldi_error = ivaldi_core::IvaldiResponse {
                                                content: None,
                                                is_error: true,
                                                ui_diffs: vec![],
                                                error: Some(error_detail),
                                                advisory: vec![],
                                            };
                                            response::openai::format_error_response(ivaldi_error)
                                                .unwrap_or_else(|_| {
                                                    json!({ "error": { "message": err, "type": "formatting_error" } })
                                                })
                                        },
                                        _ => {
                                            // MCP mode: standard JSON-RPC error
                                            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": err } })
                                        }
                                    };

                                    let mut response_string = serde_json::to_string(&error_response).unwrap();
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
