
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
use ivaldi_server::Args;

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
            // JSON-RPC notification, no response
            return (StatusCode::OK, Json(json!({"jsonrpc": "2.0", "result": null}))); 
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
        Ok(res) => (StatusCode::OK, Json(json!({ "jsonrpc": "2.0", "id": id, "result": res }))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR, // Or 200 with error object for stricter JSON-RPC compliance?
            // JSON-RPC 2.0 uses 200 OK for errors usually, but let's stick to standard practice. 
            // Actually, clients expect 200 OK + error body.
            Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": err } }))
        )
    }
}
