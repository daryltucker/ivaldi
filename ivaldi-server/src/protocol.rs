//! MCP Protocol Handler
//!
//! Handles JSON-RPC request/response protocol for MCP

use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;
use crate::state::ServerState;
use crate::tools;
use crate::tools::middleware::Middleware;
use ivaldi_server::response;
use ivaldi_core::IvaldiResponse;


/// Handle MCP initialize request
pub fn handle_initialize(
    request: &Value,
    state: &ServerState,
) -> Result<Value, String> {
    let params = request.get("params").unwrap_or(&Value::Null);
    
    // --- SESSION HOOK ---
    let session_id = params.get("initializationOptions")
        .and_then(|opts| opts.get("session_id"))
        .and_then(|v| v.as_str());
    
    // Extract project_root if provided by IDE (e.g., Mahal)
    let project_root = params.get("initializationOptions")
        .and_then(|opts| opts.get("project_root"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    
    state.session_manager().lock().unwrap()
        .load_or_create_with_root(session_id.unwrap_or("default"), None, project_root)
        .map_err(|e| format!("Failed to initialize session: {}", e))
        .map(|session| {
            info!(session_id = %session.id, root = ?session.root, "Session Attached");
            state.set_session(session.clone());
            
            json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "ivaldi-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": {}
                },
                "session": session 
            })
        })
}

/// Handle MCP tools/list request
pub fn handle_tools_list(state: &ServerState) -> Result<Value, String> {
    // Return pre-computed JSON from manual, with optional namespace prefixing
    const MANUAL_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/runtime_manual.json"));
    let mut manual: Value = serde_json::from_str(MANUAL_JSON)
        .map_err(|e| format!("Failed to parse manual: {}", e))?;

    if let Some(ns) = state.tool_namespace() {
        let clean_ns = ns.trim_matches('_');
        if !clean_ns.is_empty() {
            if let Some(tools) = manual.get_mut("tools").and_then(|t| t.as_array_mut()) {
                for tool in tools {
                    if let Some(name) = tool.get_mut("name") {
                        if let Some(s) = name.as_str() {
                            *name = json!(format!("{}_{}", clean_ns, s));
                        }
                    }
                }
            }
        }
    }

    // Return the tools list - main.rs handles JSON-RPC wrapping based on mode
    Ok(json!({ "tools": manual["tools"] }))
}

/// Handle MCP tools/call request
pub async fn handle_tools_call(
    request: &Value,
    state: &ServerState,
    cli_args: &ivaldi_server::Args,
    middleware: &std::sync::Arc<Middleware>,
) -> Result<Value, String> {
    let params = request.get("params").unwrap_or(&Value::Null);
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    
    // Strip namespace prefix if present
    let stripped_name = if let Some(ns) = state.tool_namespace() {
        let clean_ns = ns.trim_matches('_');
        if !clean_ns.is_empty() {
            name.strip_prefix(&(clean_ns.to_string() + "_")).unwrap_or(name)
        } else {
            name
        }
    } else {
        name
    };
    
    // --- CONVERSATION TRACKING ---
    // Priority: CLI args > ENV vars > IDE metadata
    let ide_conversation_id = params.get("_meta")
        .and_then(|meta| {
            meta.get("conversation_id")
                .or_else(|| meta.get("antigravity.google/conversation_id"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let conversation_id = cli_args.conversation_id.clone().or(ide_conversation_id);
    let conversation_mode = cli_args.conversation_mode;
    
    // Track conversation in session if provided
    if let (Some(conv_id), Some(session)) = (&conversation_id, state.get_session()) {
        let mut manager = state.session_manager().lock().unwrap();
        let _ = manager.track_conversation(&session.id, conv_id, conversation_mode);
    }
    
    // Log tool execution start
    let start_time = std::time::Instant::now();
    
    // Resolve path relative to Session
    let path_str = args.get("path").and_then(|v| v.as_str());
    let path_buf = if let Some(p) = path_str {
       if let Some(session) = state.get_session() {
            let manager = state.session_manager().lock().unwrap();
            manager.resolve_path(&session, std::path::Path::new(p))
       } else {
            std::path::PathBuf::from(p)
       }
    } else {
       state.get_session().map(|s| s.root).unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    };
    
    // Update args with resolved path
    let mut args = args;
    if let Some(p) = args.get_mut("path") {
        *p = serde_json::Value::String(path_buf.to_string_lossy().to_string());
    }
    
    info!("Executing tool: {} → {}", name, path_buf.display());

    // Wrap closure
    let state_clone = state.clone();
    let tool_future_closure = || async move {
         match tools::execute_tool(stripped_name, args.clone(), &state_clone).await {
            Ok(val) => {
                serde_json::from_value::<ivaldi_core::IvaldiResponse<serde_json::Value>>(val).unwrap_or_else(|e| {
                    ivaldi_core::IvaldiResponse::from_error(ivaldi_core::error::IvaldiError::Internal(format!("Serialization error: {}", e)))
                })
            },
            Err(e) => ivaldi_core::IvaldiResponse::from_error(ivaldi_core::error::IvaldiError::Internal(format!("Tool error: {}", e)))
        }
    };

    let arguments = &request["params"]["arguments"];
    let response = tools::middleware::intercept_and_execute(middleware, name, path_buf, arguments, tool_future_closure).await;
    
    // Log tool completion
    let duration = start_time.elapsed();
    info!(tool = name, duration_ms = duration.as_millis(), "Tool completed");

    // Format response based on configured mode
    // MCP Mode: Errors return JSON-RPC error objects (standard for MCP clients like opencode)
    // OpenAI Mode: Errors wrapped in result.error field (for OpenAI-compatible clients)
    // Auto Mode: Detect based on client hints, default to MCP
    let final_result = match *state.response_mode() {
        ivaldi_server::cli::ResponseMode::Mcp => response::handle_mcp_response(response),
        ivaldi_server::cli::ResponseMode::Openai => response::handle_openai_response(response),
        ivaldi_server::cli::ResponseMode::Auto => {
            let detected_mode = detect_response_mode(&request, state);
            match detected_mode {
                ivaldi_server::cli::ResponseMode::Mcp => response::handle_mcp_response(response),
                ivaldi_server::cli::ResponseMode::Openai => response::handle_openai_response(response),
                ivaldi_server::cli::ResponseMode::Auto => response::handle_mcp_response(response), // fallback to MCP
            }
        }
    };

    final_result
}

/// Auto-detect response mode based on client hints
/// Currently checks for OpenAI-style client metadata, defaults to MCP
fn detect_response_mode(request: &Value, _state: &ServerState) -> ivaldi_server::cli::ResponseMode {
    // Check for OpenAI-style client hints in request metadata
    if let Some(meta) = request.get("_meta") {
        if let Some(client_type) = meta.get("client_type").and_then(|v| v.as_str()) {
            if client_type == "openai" {
                return ivaldi_server::cli::ResponseMode::Openai;
            }
        }
    }

    // Future: Could check for other client hints or past request patterns
    // For now, default to MCP standard as it's the official protocol
    ivaldi_server::cli::ResponseMode::Mcp
}

