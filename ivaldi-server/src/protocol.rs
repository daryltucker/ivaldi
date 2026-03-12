//! MCP Protocol Handler
//!
//! Handles JSON-RPC request/response protocol for MCP

use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;
use crate::state::ServerState;
use crate::tools;
use crate::tools::middleware::Middleware;



/// Handle MCP initialize request
pub fn handle_initialize(
    request: &Value,
    state: &ServerState,
) -> Result<ivaldi_core::IvaldiResponse<Value>, String> {
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

            let init_data = json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "ivaldi-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": {}
                },
                "session": session
            });

            ivaldi_core::IvaldiResponse {
                content: Some(init_data),
                is_error: false,
                ui_diffs: vec![],
                error: None,
                advisory: vec![],
            }
        })
}

/// Handle MCP tools/list request
pub fn handle_tools_list(state: &ServerState) -> Result<ivaldi_core::IvaldiResponse<Value>, String> {
    // Return pre-computed JSON from manual, with optional namespace prefixing
    const MANUAL_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/runtime_manual.json"));
    let mut manual: Value = serde_json::from_str(MANUAL_JSON)
        .map_err(|e| format!("Failed to parse manual: {}", e))?;

    if let Some(ns) = state.tool_namespace() {
        let clean_ns = ns.trim_matches('_');
    if let Some(tools) = Some(clean_ns)
        .filter(|ns| !ns.is_empty())
        .and_then(|_| manual.get_mut("tools"))
        .and_then(|t| t.as_array_mut()) {
        for tool in tools {
            if let Some(new_name) = tool.get("name")
                .and_then(|n| n.as_str())
                .map(|s| json!(format!("{}_{}", clean_ns, s)))
                && let Some(name_ref) = tool.get_mut("name") {
                *name_ref = new_name;
            }
        }
    }
    }

    // Return tools list as IvaldiResponse (formatting happens in main.rs)
    let tools_list = json!({ "tools": manual["tools"] });

    Ok(ivaldi_core::IvaldiResponse {
        content: Some(tools_list),
        is_error: false,
        ui_diffs: vec![],
        error: None,
        advisory: vec![],
    })
}

/// Handle MCP tools/call request
pub async fn handle_tools_call(
    request: &Value,
    state: &ServerState,
    cli_args: &ivaldi_server::Args,
    middleware: &std::sync::Arc<Middleware>,
) -> Result<ivaldi_core::IvaldiResponse<Value>, String> {
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
        // Expand tilde before determining if it's absolute/relative to the session root!
        let expanded = ivaldi_core::util::path::expand_tilde(std::path::PathBuf::from(p));
        
        if let Some(session) = state.get_session() {
             let manager = state.session_manager().lock().unwrap();
             manager.resolve_path(&session, &expanded)
        } else {
             expanded
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

    // Return the raw IvaldiResponse (formatting happens in main.rs)
    Ok(response)
}



