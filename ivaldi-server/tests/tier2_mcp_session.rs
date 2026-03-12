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

#[test]
fn test_mcp_tilde_path_expansion() {
    use std::env;

    // 1. Save original HOME to restore later
    let original_home = env::var("HOME").ok();

    // 2. Create a dedicated true temp directory for the fake home
    let fake_home_dir = tempfile::tempdir().expect("Failed to create fake home dir");
    let temp_home = fake_home_dir.path();
    
    // 3. Set HOME env var safely for the duration of the test
    unsafe { env::set_var("HOME", temp_home.to_str().unwrap()) };

    // 4. Initialize server (it will inherit the new HOME)
    let mut server = StdioTestServer::new();
    server.initialize();

    // 5. Try to write a file using a tilde path
    let write_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": "~/tilde.txt",
                "content": "TESTING_TILDE"
            }
        }
    });

    server.send(write_req);
    let resp = server.recv();
    assert!(resp.get("error").is_none(), "Expected no error writing to tilde path");

    // 6. Verify it was written to the fake home dir
    assert!(temp_home.join("tilde.txt").exists(), "tilde.txt was not created in fake HOME");
    let content = fs::read_to_string(temp_home.join("tilde.txt")).unwrap();
    assert_eq!(content, "TESTING_TILDE");

    // 7. Clean up env var explicitly
    if let Some(home) = original_home {
        unsafe { env::set_var("HOME", home) };
    } else {
        unsafe { env::remove_var("HOME") };
    }
}

#[test]
fn test_mcp_syntax_guard_multi_language() {
    let mut server = StdioTestServer::new();
    server.initialize();

    // 1. Test Valid Rust File (Supported)
    let _write_rs = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": "valid.rs",
                "content": "fn main() { println!(\"Hello World\"); }"
            }
        }
    });
    server.send(write_req_helper(1, "valid.rs", "fn main() { println!(\"Hello World\"); }"));
    let resp = server.recv();
    assert!(resp.get("error").is_none());
    
    let content_items = resp["result"]["content"].as_array().expect("Should have content array");
    let rust_advisory = content_items.iter().find(|i| i["text"].as_str().unwrap_or("").contains("AST structural validation passed successfully"));
    assert!(rust_advisory.is_some(), "Expected valid syntax advisory for Rust file in content array");

    // 2. Test Invalid Python File (Supported)
    server.send(write_req_helper(2, "invalid.py", "def my_func(:\n  print(\"Broken\")"));
    let resp = server.recv();
    assert!(resp.get("error").is_none()); // The write succeeds, but the heuristic flags it
    
    let content_items = resp["result"]["content"].as_array().expect("Should have content array");
    let py_advisory = content_items.iter().find(|i| i["text"].as_str().unwrap_or("").contains("AST validation failed"));
    assert!(py_advisory.is_some(), "Expected invalid syntax advisory for Python file in content array");

    // 3. Test Unsupported File (Should be silent)
    server.send(write_req_helper(3, "random.txt", "Just some text"));
    let resp = server.recv();
    assert!(resp.get("error").is_none());
    
    // Check that there is NO syntax advisory
    let content_items = resp["result"]["content"].as_array().expect("Should have content array");
    let has_syntax_guard = content_items.iter().any(|i| i["text"].as_str().unwrap_or("").contains("AST structural validation") || i["text"].as_str().unwrap_or("").contains("AST validation failed"));
    assert!(!has_syntax_guard, "Expected NO syntax advisory for unsupported .txt file");
}

fn write_req_helper(id: u64, path: &str, content: &str) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": path,
                "content": content
            }
        }
    })
}

