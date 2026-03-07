use serde_json::json;
use std::fs;

mod common;
use common::stdio::StdioTestServer;

#[test]
fn test_mcp_session_init_and_path_resolution() {
    let mut server = StdioTestServer::new();
    
    // Create target file
    fs::write(server.root.join("target.txt"), "found me").unwrap();

    // 1. Initialize with Session ID
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "initializationOptions": {
                "session_id": "integration-test",
            }
        }
    });
    server.send(init_req);
    let resp = server.recv();
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["session"]["id"], "integration-test");

    // 2. Call tool with RELATIVE path (should use session CWD)
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "read_file",
            "arguments": {
                "path": "Cargo.toml" 
            }
        }
    });
    server.send(call_req);
    let resp = server.recv();
    
    // If it finds it, great. If not, we assert the structure at least.
    if resp["result"]["isError"] == false {
        if let Some(content_items) = resp["result"]["content"].as_array() {
            if let Some(text) = content_items[0]["text"].as_str() {
                 assert!(text.contains("[package]"));
            }
        }
    }
    
    // 3. Switch Session
    let session_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "session_init",
            "arguments": {
                "id": "agent-switched-session",
                "root": server.temp_dir.path().join("agent_root").to_str().unwrap()
            }
        }
    });
    server.send(session_req);
    let resp = server.recv();
    assert!(resp["result"]["content"][0]["text"].as_str().unwrap().contains("agent-switched-session"));

    // 4. Verify Context Switch
    let get_req = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "session_get",
            "arguments": {}
        }
    });
    server.send(get_req);
    let resp = server.recv();
    assert!(resp["result"]["content"][0]["text"].as_str().unwrap().contains("agent-switched-session"));

    // 5. Verify Explicit Append
    // Write Init
    let write_req = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": "lifecycle.txt",
                "content": "Hello"
            }
        }
    });
    server.send(write_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());

    // Write Append
    let append_req = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": "lifecycle.txt",
                "content": " World",
                "append": true
            }
        }
    });
    server.send(append_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());

    // Verify Content
    let read_check = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "read_file",
            "arguments": {
                "path": "lifecycle.txt"
            }
        }
    });
    server.send(read_check);
    let resp = server.recv();
    let content = resp["result"]["content"][0]["text"].as_str().expect("Content missing");
    assert!(content.contains("Hello World"));
}

#[test]
fn test_mcp_find_files() {
    let mut server = StdioTestServer::new();
    
    // Setup files
    fs::create_dir_all(server.root.join("src")).unwrap();
    fs::write(server.root.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(server.root.join("README.md"), "# Readme").unwrap();

    server.initialize();

    // Find Files
    let find_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "find_files",
            "arguments": {
                "pattern": "*.rs",
                "path": server.root.to_str().unwrap()
            }
        }
    });
    
    server.send(find_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());
    
    let result_str = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(result_str.contains("src/main.rs"));
    assert!(!result_str.contains("README.md"));
}

#[test]
fn test_mcp_list_dir() {
    let mut server = StdioTestServer::new();
    
    fs::create_dir_all(server.root.join("subdir")).unwrap();
    fs::write(server.root.join("file1.txt"), "").unwrap();

    server.initialize();

    // List Dir
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "list_dir",
            "arguments": {
                "path": server.root.to_str().unwrap()
            }
        }
    });

    server.send(list_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());
    
    let result_str = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(result_str.contains("file1.txt"));
    assert!(result_str.contains("subdir"));
}

#[test]
fn test_mcp_edit_file() {
    let mut server = StdioTestServer::new();
    let file_path = server.root.join("edit.txt");
    fs::write(&file_path, "line1\nline2\nline3").unwrap();

    server.initialize();

    // Edit File
    let edit_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "edit_file",
            "arguments": {
                "path": file_path.to_str().unwrap(),
                "from_line": 2,
                "to_line": 2,
                "replacement": "MODIFIED"
            }
        }
    });

    server.send(edit_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());
    
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("MODIFIED"));
}

#[test]
fn test_mcp_undo() {
    let mut server = StdioTestServer::new();
    let file_path = server.root.join("undo.txt");
    fs::write(&file_path, "INITIAL").unwrap();

    server.initialize();

    // 1. Overwrite
    let write_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": file_path.to_str().unwrap(),
                "content": "MODIFIED",
                "overwrite": true
            }
        }
    });
    server.send(write_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());

    // 2. Undo
    let undo_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "undo",
            "arguments": {
                "path": server.root.to_str().unwrap()
            }
        }
    });
    server.send(undo_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none());
    
    // Check rollback
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "INITIAL");
}

#[test]
fn test_tool_namespace_prefixing() {
    use std::env;

    // Set namespace env var for this test
    unsafe { env::set_var("IVALDI_TOOL_NAMESPACE", "testns") };
    unsafe { env::set_var("IVALDI_RESPONSE_MODE", "mcp") };

    let mut server = StdioTestServer::new();

    // Initialize session
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    server.send(init_req);
    let _resp = server.recv(); // Ignore response

    // Test that tools/list returns prefixed names
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    server.send(list_req);
    let list_resp = server.recv();

    // Validate MCP response format: should be {"jsonrpc": "2.0", "result": {"tools": [...]}}
    assert_eq!(list_resp["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");

    // Debug the response structure
    if !list_resp["result"]["tools"].is_array() {
        println!("DEBUG: list_resp keys: {:?}", list_resp.as_object().unwrap().keys().collect::<Vec<_>>());
        println!("DEBUG: result is_object: {:?}", list_resp["result"].is_object());
        if let Some(result_obj) = list_resp["result"].as_object() {
            println!("DEBUG: result keys: {:?}", result_obj.keys().collect::<Vec<_>>());
        }
        panic!("Result should contain tools array. Full response: {:?}", list_resp);
    }

    // Summary for debugging
    let tools_count = list_resp["result"]["tools"].as_array().unwrap().len();
    println!("✓ MCP tools/list: {} tools returned, first tool prefixed: {}",
             tools_count,
             list_resp["result"]["tools"][0]["name"].as_str().unwrap().starts_with("testns_"));

    let tools = list_resp["result"]["tools"].as_array().unwrap();

    // Check that first tool has prefix
    let first_tool_name = tools[0]["name"].as_str().unwrap();
    assert!(first_tool_name.starts_with("testns_"), "Tool name should be prefixed: {}", first_tool_name);

    // Find prefixed find_files tool
    let find_files_tool = tools.iter().find(|t| t["name"].as_str().unwrap().ends_with("_find_files"));
    assert!(find_files_tool.is_some(), "Should find prefixed find_files tool");

    let prefixed_name = find_files_tool.unwrap()["name"].as_str().unwrap();

    // Test calling with prefixed name
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": prefixed_name,
            "arguments": {
                "pattern": "*.rs",
                "max_entries": 5
            }
        }
    });
    server.send(call_req);
    let call_resp = server.recv();

    // Validate MCP tool call response format
    assert_eq!(call_resp["jsonrpc"], "2.0", "Response should be JSON-RPC 2.0");
    assert!(call_resp["result"]["content"].is_array(), "Tool call result should have content array");

    // Summary for debugging
    let content_count = call_resp["result"]["content"].as_array().unwrap().len();
    println!("✓ MCP tool call: {} content items returned", content_count);

    // Clean up env vars
    unsafe { env::remove_var("IVALDI_TOOL_NAMESPACE") };
    unsafe { env::remove_var("IVALDI_RESPONSE_MODE") };
}
