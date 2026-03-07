use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn test_read_files_multi_content() {
    let mut child = Command::new("cargo")
        .args(["run", "-p", "ivaldi-server", "--", "--transport", "stdio"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn ivaldi-server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // 1. Initialize
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" },
            "initializationOptions": {
                "session_id": "test-multi",
                "project_root": env!("CARGO_MANIFEST_DIR")
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line.clear();

    // 2. Call read_files (plural)
    let call_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "read_files",
            "arguments": {
                "paths": ["Cargo.toml"]
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();

    reader.read_line(&mut line).unwrap();
    drop(stdin);
    child.kill().ok();

    let response: Value = serde_json::from_str(&line).expect("Failed to parse response");
    let result = response.get("result").expect("No result field");
    let content = result.get("content").expect("No content field").as_array().expect("content is not array");
    let first_item = &content[0];
    let text = first_item.get("text").expect("No text field").as_str().expect("text is not string");

    println!("DEBUG MULTI TEXT: {}", text);

    // Should be pretty-printed JSON because it's a map
    assert!(text.trim().starts_with('{'), "Should be JSON for read_files result");
    assert!(text.contains("results"), "Should contain results key");
}
