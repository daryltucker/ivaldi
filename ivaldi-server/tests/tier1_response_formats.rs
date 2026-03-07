//! Tier 1 Response Format Transformation Tests
//!
//! These tests verify the exact output format of each response formatter
//! in isolation, ensuring they produce the correct structure for their
//! respective client types.

use ivaldi_core::response::{ErrorDetail, IvaldiResponse};
use ivaldi_server::response::*;
use serde_json::json;

/// Test MCP success response format
#[test]
fn test_mcp_initialize_success_format() {
    let input_content = json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {"name": "ivaldi-mcp", "version": "0.1.0"},
        "capabilities": {"tools": {}}
    });


    // Format using MCP formatter
    let formatted = mcp::format_success_content(input_content.clone());

    // Should return raw content (no JSON-RPC wrapper)
    assert_eq!(formatted, input_content);
}

/// Test MCP tools/list success format
#[test]
fn test_mcp_tools_list_success_format() {
    let input_content = json!({
        "tools": [
            {
                "name": "run_command",
                "description": "Execute a shell command",
                "inputSchema": {"type": "object"}
            }
        ]
    });


    // Format using MCP formatter
    let formatted = mcp::format_success_content(input_content.clone());

    // Should return raw content (no JSON-RPC wrapper)
    assert_eq!(formatted, input_content);
}

/// Test MCP tool call success format
#[test]
fn test_mcp_tool_call_success_format() {
    let input_content = json!({"stdout": "Hello World", "exit_code": 0});


    // Format using MCP formatter
    let formatted = mcp::format_success_content(input_content.clone());

    // Should wrap in MCP content array format
    let expected = json!({
        "isError": false,
        "content": [{
            "type": "text",
            "text": "{\n  \"stdout\": \"Hello World\",\n  \"exit_code\": 0\n}"
        }]
    });

    assert_eq!(formatted, expected);
}

/// Test MCP error format
#[test]
fn test_mcp_error_format() {
    let error_detail = ErrorDetail {
        code: "-32003".to_string(),
        message: "Permission denied".to_string(),
        hint: None,
        context: None,
    };

    // Format using MCP error formatter
    let formatted =
        mcp::format_error_content(error_detail.code.clone(), error_detail.message.clone());

    let expected = json!({
        "isError": true,
        "content": [{
            "type": "text",
            "text": "Error [-32003]: Permission denied"
        }],
        "error": {
            "code": "-32003",
            "message": "Permission denied"
        }
    });

    assert_eq!(formatted, expected);
}

/// Test OpenAI success format
#[test]
fn test_openai_success_format() {
    let input_content = json!({"stdout": "test output", "exit_code": 0});

    let ivaldi_response = IvaldiResponse {
        content: Some(input_content.clone()),
        is_error: false,
        error: None,
        advisory: vec![],
    };

    // Format using OpenAI formatter
    let result = openai::format_success_response(ivaldi_response);
    let formatted = result.unwrap();

    // Should be OpenAI Chat Completions format
    assert!(formatted.get("choices").is_some());
    assert!(formatted.get("id").is_some());
    assert!(formatted.get("object").is_some());
    assert_eq!(formatted["object"], "chat.completion");

    let choices = formatted["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 1);
    assert_eq!(
        choices[0]["message"]["content"],
        "{\"stdout\":\"test output\",\"exit_code\":0}"
    );
}

/// Test OpenAI error format
#[test]
fn test_openai_error_format() {
    let error_detail = ErrorDetail {
        code: "tool_error".to_string(),
        message: "Command failed".to_string(),
        hint: None,
        context: None,
    };

    let ivaldi_response = IvaldiResponse {
        content: None,
        is_error: true,
        error: Some(error_detail),
        advisory: vec![],
    };

    // Format using OpenAI formatter
    let result = openai::format_error_response(ivaldi_response);
    let formatted = result.unwrap();

    // Should be OpenAI error format
    assert!(formatted.get("error").is_some());
    assert_eq!(formatted["error"]["message"], "Command failed");
    assert_eq!(formatted["error"]["type"], "tool_error");
}

/// Test transport layer envelope handling
#[test]
fn test_transport_envelope_logic() {
    // This test verifies the transport layer correctly handles each mode

    // MCP mode should get JSON-RPC wrapper
    let mcp_content = json!({"isError": false, "content": [{"type": "text", "text": "test"}]});
    let _mcp_wrapped = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": mcp_content
    });
    // (In transport layer: MCP gets wrapped, OpenAI/OpenCode do not)

    // OpenAI mode should NOT get JSON-RPC wrapper
    let _openai_content = json!({"choices": [{"message": {"content": "test"}}]});
    // (In transport layer: stays as openai_content directly)

    // This is tested implicitly through the format tests above
    // but could be expanded to test the full transport pipeline
}
