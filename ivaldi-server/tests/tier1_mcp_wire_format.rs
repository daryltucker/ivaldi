//! Tier 1: MCP Wire Format Tests
//!
//! These tests validate the EXACT JSON-RPC output that ivaldi-server produces.
//! They spawn the actual server binary and test real stdio communication.
//!
//! If these tests fail, MCP clients like Claude Desktop WILL NOT WORK.
//! DO NOT ignore failures in this file.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Helper: spawn ivaldi-server and send a request, return the response
fn send_mcp_request(request: &Value) -> Value {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("sessions.toml");

    let mut child = Command::new("ivaldi-server")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("IVALDI_CONFIG", config_path.to_str().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ivaldi-server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");

    // Send the request
    let request_str = serde_json::to_string(request).unwrap();
    writeln!(stdin, "{}", request_str).expect("Failed to write to stdin");
    drop(stdin); // Close stdin to signal EOF

    // Read the response
    let reader = BufReader::new(stdout);
    let mut response_line = String::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if !line.trim().is_empty() {
                response_line = line;
                break;
            }
        }
    }

    child.kill().ok();

    serde_json::from_str(&response_line).expect(&format!(
        "Failed to parse response as JSON: {}",
        response_line
    ))
}

/// Test: MCP initialize request returns proper JSON-RPC response
#[test]
fn test_mcp_initialize_returns_jsonrpc_envelope() {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            },
            "initializationOptions": {
                "session_id": "test-session",
                "project_root": env!("CARGO_MANIFEST_DIR")
            }
        }
    });

    let response = send_mcp_request(&request);

    // MUST have jsonrpc field
    assert_eq!(
        response.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "Response must have jsonrpc: '2.0'. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have matching id
    assert_eq!(
        response.get("id").and_then(|v| v.as_i64()),
        Some(1),
        "Response must have id: 1. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have result field (not error)
    assert!(
        response.get("result").is_some(),
        "Response must have 'result' field. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // result MUST contain protocolVersion
    let result = response.get("result").unwrap();
    assert!(
        result.get("protocolVersion").is_some(),
        "result must contain 'protocolVersion'. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // result MUST contain serverInfo
    assert!(
        result.get("serverInfo").is_some(),
        "result must contain 'serverInfo'. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // result MUST contain capabilities
    assert!(
        result.get("capabilities").is_some(),
        "result must contain 'capabilities'. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );
}

/// Test: MCP tools/list request returns proper JSON-RPC response
#[test]
fn test_mcp_tools_list_returns_jsonrpc_envelope() {
    // First initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" },
            "initializationOptions": {
                "session_id": "test-session-tools-list",
                "project_root": env!("CARGO_MANIFEST_DIR")
            }
        }
    });

    let mut child = Command::new("ivaldi-server")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ivaldi-server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // Send initialize
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();

    // Read initialize response
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line.clear();

    // Send tools/list
    let tools_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    writeln!(stdin, "{}", serde_json::to_string(&tools_request).unwrap()).unwrap();

    // Read tools/list response
    reader.read_line(&mut line).unwrap();
    drop(stdin);
    child.kill().ok();

    let response: Value = serde_json::from_str(&line)
        .expect(&format!("Failed to parse tools/list response: {}", line));

    // MUST have jsonrpc field
    assert_eq!(
        response.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "Response must have jsonrpc: '2.0'. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have matching id
    assert_eq!(
        response.get("id").and_then(|v| v.as_i64()),
        Some(2),
        "Response must have id: 2. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have result field
    assert!(
        response.get("result").is_some(),
        "Response must have 'result' field. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // result MUST contain tools array
    let result = response.get("result").unwrap();
    assert!(
        result.get("tools").is_some() && result.get("tools").unwrap().is_array(),
        "result must contain 'tools' array. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // tools array MUST have at least one tool
    let tools = result.get("tools").unwrap().as_array().unwrap();
    assert!(!tools.is_empty(), "tools array must not be empty");

    // Each tool MUST have name, description, inputSchema
    for tool in tools {
        assert!(
            tool.get("name").is_some(),
            "Each tool must have 'name'. Got: {}",
            serde_json::to_string_pretty(&tool).unwrap()
        );
        assert!(
            tool.get("description").is_some(),
            "Each tool must have 'description'. Got: {}",
            serde_json::to_string_pretty(&tool).unwrap()
        );
        assert!(
            tool.get("inputSchema").is_some(),
            "Each tool must have 'inputSchema'. Got: {}",
            serde_json::to_string_pretty(&tool).unwrap()
        );
    }
}

/// Test: MCP tools/call success returns proper JSON-RPC response with content array
#[test]
fn test_mcp_tools_call_success_returns_jsonrpc_with_content() {
    let mut child = Command::new("ivaldi-server")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ivaldi-server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" },
            "initializationOptions": {
                "session_id": "test-session-tools-call-success",
                "project_root": env!("CARGO_MANIFEST_DIR")
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line.clear();

    // Call a simple tool (read_file on a file that exists)
    let call_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "read_file",
            "arguments": {
                "path": "../Cargo.toml"
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();

    reader.read_line(&mut line).unwrap();
    drop(stdin);
    child.kill().ok();

    let response: Value = serde_json::from_str(&line)
        .expect(&format!("Failed to parse tools/call response: {}", line));

    // MUST have jsonrpc field
    assert_eq!(
        response.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "Response must have jsonrpc: '2.0'. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have matching id
    assert_eq!(
        response.get("id").and_then(|v| v.as_i64()),
        Some(3),
        "Response must have id: 3. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have result field (not error at top level)
    assert!(
        response.get("result").is_some(),
        "Response must have 'result' field. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    let result = response.get("result").unwrap();

    // result MUST have isError field set to false
    assert_eq!(
        result.get("isError").and_then(|v| v.as_bool()),
        Some(false),
        "result.isError must be false for success. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // result MUST have content array
    assert!(
        result.get("content").is_some() && result.get("content").unwrap().is_array(),
        "result must have 'content' array. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    let content = result.get("content").unwrap().as_array().unwrap();
    assert!(!content.is_empty(), "content array must not be empty");

    // First content item must have type and text
    let first_content = &content[0];
    assert_eq!(
        first_content.get("type").and_then(|v| v.as_str()),
        Some("text"),
        "content[0].type must be 'text'. Got: {}",
        serde_json::to_string_pretty(&first_content).unwrap()
    );
    assert!(
        first_content.get("text").is_some(),
        "content[0] must have 'text' field. Got: {}",
        serde_json::to_string_pretty(&first_content).unwrap()
    );
}

/// Test: MCP tools/call error returns proper JSON-RPC response with isError: true
#[test]
fn test_mcp_tools_call_error_returns_jsonrpc_with_is_error() {
    let mut child = Command::new("ivaldi-server")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ivaldi-server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" },
            "initializationOptions": {
                "session_id": "test-session-tools-call-error",
                "project_root": env!("CARGO_MANIFEST_DIR")
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line.clear();

    // Call read_file on a file that doesn't exist
    let call_request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "read_file",
            "arguments": {
                "path": "/nonexistent/file/that/does/not/exist.txt"
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();

    reader.read_line(&mut line).unwrap();
    drop(stdin);
    child.kill().ok();

    let response: Value = serde_json::from_str(&line).expect(&format!(
        "Failed to parse tools/call error response: {}",
        line
    ));

    // MUST have jsonrpc field
    assert_eq!(
        response.get("jsonrpc").and_then(|v| v.as_str()),
        Some("2.0"),
        "Response must have jsonrpc: '2.0'. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have matching id
    assert_eq!(
        response.get("id").and_then(|v| v.as_i64()),
        Some(4),
        "Response must have id: 4. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // MUST have result field (tool errors go in result, not top-level error)
    assert!(
        response.get("result").is_some(),
        "Response must have 'result' field for tool errors. Got: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    let result = response.get("result").unwrap();

    // result MUST have isError field set to true
    assert_eq!(
        result.get("isError").and_then(|v| v.as_bool()),
        Some(true),
        "result.isError must be true for errors. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // result MUST have error object with code and message
    assert!(
        result.get("error").is_some(),
        "result must have 'error' object. Got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    let error = result.get("error").unwrap();
    assert!(
        error.get("message").is_some(),
        "error must have 'message'. Got: {}",
        serde_json::to_string_pretty(&error).unwrap()
    );
}
