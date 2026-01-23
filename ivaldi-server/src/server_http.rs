
use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use crate::state::ServerState;
use crate::tools::middleware::Middleware;
use crate::protocol;
use crate::response;
use ivaldi_server::Args;
use ivaldi_core::response::ErrorDetail;

#[derive(Clone)]
struct AppState {
    server_state: ServerState,
    middleware: Arc<Middleware>,
    args: Args,
}

pub async fn run(port: u16, state: ServerState, middleware: Arc<Middleware>, args: Args) -> anyhow::Result<()> {
    let app_state = AppState {
        server_state: state,
        middleware,
        args,
    };

    let app = Router::new()
        .route("/mcp", post(handle_mcp_request))
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting HTTP server on {}", addr);
    
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    info!("Listening on {}", local_addr);
    axum::serve(listener, app).await?;

    Ok(())
}

#[axum::debug_handler]
async fn handle_mcp_request(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let id = request.get("id").unwrap_or(&Value::Null).clone();
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
    
    // TODO: Reuse logic from main.rs loop
    // Ideally refactor main.rs loop body into a generic `process_request` function
    // For now, I'll allow some duplication or I will refactor protocol.rs to handle the dispatch fully.
    
    // Refactoring plan: I will call protocol::handle_dispatch if I move the dispatch logic there.
    // But currently main.rs handles the switch.
    // Let's implement the switch here for parity.

    let result = match method {
        "initialize" => protocol::handle_initialize(&request, &state.server_state),
        "notifications/initialized" => {
            // JSON-RPC notification, no response needed, but return success
            let success_response = ivaldi_core::IvaldiResponse {
                content: Some(json!(null)),
                is_error: false,
                error: None,
                advisory: vec![],
            };
            return (StatusCode::OK, Json(json!({ "jsonrpc": "2.0", "id": id, "result": success_response.content })));
        },
        "tools/list" => protocol::handle_tools_list(&state.server_state),
        "tools/call" => protocol::handle_tools_call(&request, &state.server_state, &state.args, &state.middleware).await,
        _ => {
            if id.is_null() { 
                 return (StatusCode::OK, Json(json!({"jsonrpc": "2.0", "result": null}))); 
            }
            Err(format!("Method not found: {}", method))
        }
    };

    match result {
        Ok(ivaldi_response) => {
            // Format response based on mode using the formatter system
            let _registry = response::get_registry();
            let is_error = ivaldi_response.is_error;
            let formatted_result = match *state.server_state.response_mode() {
                ivaldi_server::cli::ResponseMode::Mcp => {
                    if is_error {
                        // Use new MCP module for error formatting
                        let error_detail = ivaldi_response.error.as_ref().unwrap();
                        Ok(response::mcp::format_error_content(
                            error_detail.code.clone(),
                            error_detail.message.clone()
                        ))
                    } else {
                        // Use new MCP module for success formatting
                        Ok(response::mcp::format_success_content(
                            ivaldi_response.content.unwrap_or(Value::Null)
                        ))
                    }
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
                        if is_error {
                            let error_detail = ivaldi_response.error.as_ref().unwrap();
                            Ok(response::mcp::format_error_content(
                                error_detail.code.clone(),
                                error_detail.message.clone()
                            ))
                        } else {
                            Ok(response::mcp::format_success_content(
                                ivaldi_response.content.unwrap_or(Value::Null)
                            ))
                        }
                    }
                }
            }.unwrap_or_else(|err| {
                json!({ "error": { "message": format!("{}", err), "type": "formatting_error" } })
            });

            // Wrap MCP responses in JSON-RPC envelope, leave OpenAI/OpenCode as-is
            let response_to_send = match *state.server_state.response_mode() {
                ivaldi_server::cli::ResponseMode::Mcp | ivaldi_server::cli::ResponseMode::Auto => {
                    if is_error {
                        // For MCP errors, use JSON-RPC error format
                        json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id").unwrap_or(&serde_json::Value::Null),
                            "error": {
                                "code": formatted_result["error"]["code"].as_str().unwrap_or("-32000").parse().unwrap_or(-32000),
                                "message": formatted_result["error"]["message"].as_str().unwrap_or("Unknown error")
                            }
                        })
                    } else {
                        json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id").unwrap_or(&serde_json::Value::Null),
                            "result": formatted_result
                        })
                    }
                },
                _ => formatted_result
            };

            (StatusCode::OK, Json(response_to_send))
        },
        Err(err) => {
            // Format error response based on mode
            let _registry = response::get_registry();
            let error_response = match *state.server_state.response_mode() {
                ivaldi_server::cli::ResponseMode::Openai => {
                    // OpenAI/OpenCode mode: format error using appropriate formatter
                    let formatter_name = "openai";
                    if let Some(formatter) = _registry.get_formatter(formatter_name) {
                        let error_detail = ivaldi_core::response::ErrorDetail {
                            code: "tool_error".to_string(),
                            message: err.clone(),
                            hint: None,
                            context: None,
                        };
                        let ivaldi_error = ivaldi_core::IvaldiResponse {
                            content: None,
                            is_error: true,
                            error: Some(error_detail),
                            advisory: vec![],
                        };
                        formatter.format_error(ivaldi_error).unwrap_or_else(|_| {
                            json!({ "error": { "message": err, "type": "formatting_error" } })
                        })
                    } else {
                        json!({ "error": { "message": err, "type": "formatter_not_found" } })
                    }
                },
                _ => {
                    // MCP mode: standard JSON-RPC error
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": err } })
                }
            };

            (StatusCode::OK, Json(error_response))
        }
    }
}
