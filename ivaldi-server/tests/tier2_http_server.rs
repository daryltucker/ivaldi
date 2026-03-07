//! Tier 2 HTTP Server Integration Tests
//!
//! Tests the ivaldi-server HTTP transport mode with real HTTP requests.
//! Covers write_file, edit_file, read_file via HTTP POST to /mcp

use serde_json::json;
use std::fs;

mod common;
use common::http::TestServer;

#[tokio::test]
async fn test_http_write_file() {
    let server = TestServer::new();
    let client = server.client();
    
    server.initialize(&client).await;
    
    let target_path = server.root().join("http_test.txt");
    let args = json!({
        "path": target_path.to_str().unwrap(),
        "content": "Hello from HTTP test"
    });

    server.call_tool(&client, "write_file", args).await;
    
    let content = fs::read_to_string(&target_path).expect("File should exist");
    assert_eq!(content, "Hello from HTTP test");
}

#[tokio::test]
async fn test_http_read_file() {
    let server = TestServer::new();
    let test_file = server.root().join("read_test.txt");
    fs::write(&test_file, "Test content here").unwrap();
    
    let client = server.client();
    server.initialize(&client).await;
    
    let args = json!({ "path": test_file.to_str().unwrap() });
    let body = server.call_tool(&client, "read_file", args).await;

    // The response is MCP format: body["result"]["content"][0]["text"] contains raw text
    let text_content = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text_content.contains("Test content here"));
}

#[tokio::test]
async fn test_http_edit_file() {
    let server = TestServer::new();
    let test_file = server.root().join("edit_test.txt");
    fs::write(&test_file, "line1\nline2\nline3\n").unwrap();
    
    let client = server.client();
    server.initialize(&client).await;
    
    let args = json!({
        "path": test_file.to_str().unwrap(),
        "from_line": 2,
        "to_line": 2,
        "replacement": "MODIFIED"
    });
    
    server.call_tool(&client, "edit_file", args).await;
    
    let content = fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("MODIFIED"));
    assert!(content.contains("line1"));
    assert!(content.contains("line3"));
}

#[tokio::test]
async fn test_http_find_files() {
    let server = TestServer::new();
    fs::create_dir_all(server.root().join("src")).unwrap();
    fs::write(server.root().join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(server.root().join("src/lib.rs"), "pub fn test() {}").unwrap();
    fs::write(server.root().join("README.md"), "# Readme").unwrap();
    
    let client = server.client();
    server.initialize(&client).await;
    
    let args = json!({
        "pattern": "*.rs",
        "path": server.root().to_str().unwrap()
    });
    
    let body = server.call_tool(&client, "find_files", args).await;
    let result_str = body["result"]["content"][0]["text"].as_str().unwrap();
    
    assert!(result_str.contains("src/main.rs"));
    assert!(result_str.contains("src/lib.rs"));
    assert!(!result_str.contains("README.md"));
}

#[tokio::test]
async fn test_http_list_dir() {
    let server = TestServer::new();
    fs::create_dir_all(server.root().join("subdir")).unwrap();
    fs::write(server.root().join("file1.txt"), "content").unwrap();
    fs::write(server.root().join("subdir/file2.txt"), "content").unwrap();
    
    let client = server.client();
    server.initialize(&client).await;
    
    let args = json!({ "path": server.root().to_str().unwrap() });
    let body = server.call_tool(&client, "list_dir", args).await;
    let result_str = body["result"]["content"][0]["text"].as_str().unwrap();
    
    assert!(result_str.contains("file1.txt"));
    assert!(result_str.contains("subdir"));
}

#[tokio::test]
async fn test_http_undo() {
    let server = TestServer::new();
    let target_path = server.root().join("undo_test.txt");
    fs::write(&target_path, "INITIAL").unwrap();
    
    let client = server.client();
    server.initialize(&client).await;
    
    // 1. Modify
    let write_args = json!({
        "path": target_path.to_str().unwrap(),
        "content": "MODIFIED",
        "overwrite": true
    });
    server.call_tool(&client, "write_file", write_args).await;
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "MODIFIED");
    
    // 2. Undo
    let undo_args = json!({ "path": server.root().to_str().unwrap() });
    server.call_tool(&client, "undo", undo_args).await;
    
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "INITIAL");
}

#[tokio::test]
async fn test_http_session_lifecycle() {
    let server = TestServer::new();
    let client = server.client();
    server.initialize(&client).await;
    
    // 2. Session Init
    let args = json!({
        "id": "test-session",
        "root": server.root().to_str().unwrap()
    });
    server.call_tool(&client, "session_init", args).await;

    // 3. Session Get
    let body = server.call_tool(&client, "session_get", json!({})).await;
    let result = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(result.contains("test-session"));

    // 4. Session Update
    let args = json!({
        "label": "Test Session Label",
        "add_tags": ["test", "http"]
    });
    server.call_tool(&client, "session_update", args).await;

    // 5. Session List
    let body = server.call_tool(&client, "session_list", json!({})).await;
    let result = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(result.contains("test-session") || result.contains("integration-test"), "Result: {}", result);
    assert!(result.contains("Test Session Label"), "Result: {}", result);
}

#[tokio::test]
async fn test_http_error_handling() {
    let server = TestServer::new();
    let client = server.client();
    server.initialize(&client).await;
    
    let args = json!({});
    // Manually making a bad request since call_tool asserts success
     let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "nonexistent_tool",
            "arguments": args
        }
    });
    
    let resp = client.post(&server.url())
        .json(&req)
        .send()
        .await
        .expect("Request should complete");
        
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some() || body["result"]["isError"] == true);
}
