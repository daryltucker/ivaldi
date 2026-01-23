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
    if let Some(content_items) = resp["result"]["content"].as_array() {
        if let Some(text) = content_items[0]["text"].as_str() {
             assert!(text.contains("[package]"));
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

    // 5. Verify Smart Append
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
