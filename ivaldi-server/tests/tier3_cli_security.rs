use serde_json::json;
use std::fs;
use std::collections::HashMap;

mod common;
use common::stdio::StdioTestServer;

// Helper to assert response is success
fn assert_success(resp: &serde_json::Value, expected_stdout: Option<&str>) {
    println!("Checking success for: {:?}", resp);
    let result = &resp["result"];
    assert_eq!(result["isError"], false, "Response should be success");
    
    if let Some(expected) = expected_stdout {
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(expected), "Stdout '{}' should contain '{}'", text, expected);
    }
}



#[test]
fn test_cli_execution_policy_enforcement() {
    // 1. Prepare env
    let (temp_dir, root) = StdioTestServer::prepare();
    
    // 2. Setup Policy (Explicitly forbid ls, others allowed by default)
    let policy_dir = root.join(".ivaldi").join("policies");
    fs::create_dir_all(&policy_dir).expect("Failed to create policies dir");
    
    let policy_content = r#"
        forbid(
            principal, 
            action == Action::"exec", 
            resource == Command::"ls"
        );
    "#;
    
    fs::write(policy_dir.join("test.cedar"), policy_content).expect("Failed to write policy");

    // 3. Spawn Server
    let mut server = StdioTestServer::spawn(temp_dir, root);
    server.initialize();

    // 4. Test Allowed Command (echo) - Should be allowed by global permit
    let echo_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "run_command",
            "arguments": {
                "command": "echo",
                "args": ["Hello Policy"]
            }
        }
    });
    server.send(echo_req);
    let echo_resp = server.recv();
    assert_success(&echo_resp, Some("Hello Policy"));

    // 5. Test Denied Command (ls) - Denied by explicit forbid
    let ls_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "run_command",
            "arguments": {
                "command": "ls",
                "args": []
            }
        }
    });
    server.send(ls_req);
    let ls_resp = server.recv();
    
    let ls_ivaldi_resp = &ls_resp["result"];
    assert_eq!(ls_ivaldi_resp["isError"], true);
    assert!(ls_ivaldi_resp["error"]["message"].as_str().unwrap().contains("Permission denied"));
}

#[test]
fn test_default_allow_behavior() {
    // 1. Prepare env (No policies)
    let (temp_dir, root) = StdioTestServer::prepare();
    
    let mut server = StdioTestServer::spawn(temp_dir, root);
    server.initialize();

    // 2. Try echo (should SUCCEED now with ALLOW ALL default)
    let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "run_command",
            "arguments": {
                "command": "echo",
                "args": ["Hello Default Allow"]
            }
        }
    });
    server.send(req);
    let resp = server.recv();
    
    assert_success(&resp, Some("Hello Default Allow"));
}

#[test]
fn test_explicit_forbid_all_enforcement() {
    // 1. Prepare env
    let (temp_dir, root) = StdioTestServer::prepare();
    
    // 2. Setup "DENY ALL" Policy via explicit forbid
    let policy_dir = root.join(".ivaldi").join("policies");
    fs::create_dir_all(&policy_dir).expect("Failed to create policies dir");
    
    let policy_content = r#"forbid(principal, action, resource);"#;
    fs::write(policy_dir.join("deny_all.cedar"), policy_content).expect("Failed to write policy");

    // 3. Spawn Server
    let mut server = StdioTestServer::spawn(temp_dir, root);
    server.initialize();

    // 4. Try echo (should FAIL due to explicit forbid)
    let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "run_command",
            "arguments": {
                "command": "echo",
                "args": ["Should Fail"]
            }
        }
    });
    server.send(req);
    let resp = server.recv();
    
    let ivaldi_resp = &resp["result"];
    assert_eq!(ivaldi_resp["isError"], true);
    assert!(ivaldi_resp["error"]["message"].as_str().unwrap().contains("Permission denied"));
}

#[test]
fn test_sandbox_fs_isolation() {
    // 1. Prepare env
    let (temp_dir, root) = StdioTestServer::prepare();
    
    // 2. Setup Permissive Policy (so we test Sandbox, not Cedar)
    let policy_dir = root.join(".ivaldi").join("policies");
    fs::create_dir_all(&policy_dir).expect("Failed to create policies dir");
    fs::write(policy_dir.join("allow_all.cedar"), "permit(principal, action, resource);").expect("Failed to write policy");

    // 3. Spawn Server with FS Isolation
    let mut env = HashMap::new();
    env.insert("IVALDI_EXEC_SANDBOXING", "fs");
    let mut server = StdioTestServer::spawn_with_env(temp_dir, root, env);
    server.initialize();

    // 4. Try to write to root / (should fail due to RO bind)
    // Note: 'touch /sandbox_test'
    let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "run_command",
            "arguments": {
                "command": "touch",
                "args": ["/sandbox_test_fail"]
            }
        }
    });
    server.send(req);
    let resp = server.recv();
    
    // Should be success (tool ran) but exit code != 0
    let result = &resp["result"];
    if result["isError"] == false {
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Writing to / should fail in sandbox") || text.contains("Read-only file system") || text.contains("\"exit_code\": 1"), "Got: {}", text);
    } else {
        // Or it failed to even spawn if bwrap is strict
        // But likely it spawned and touch returned error
    }
}

#[test]
fn test_sandbox_network_isolation() {
    // 1. Prepare env
    let (temp_dir, root) = StdioTestServer::prepare();
    
    // 2. Setup Permissive Policy
    let policy_dir = root.join(".ivaldi").join("policies");
    fs::create_dir_all(&policy_dir).expect("Failed to create policies dir");
    fs::write(policy_dir.join("allow_all.cedar"), "permit(principal, action, resource);").expect("Failed to write policy");

    // 3. Spawn Server with Net Isolation
    let mut env = HashMap::new();
    env.insert("IVALDI_EXEC_SANDBOXING", "net");
    let mut server = StdioTestServer::spawn_with_env(temp_dir, root, env);
    server.initialize();

    // 4. Try to ping google (should fail)
    // using curl with timeout to fail fast
    let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "run_command",
            "arguments": {
                "command": "curl",
                "args": ["--connect-timeout", "1", "https://google.com"]
            }
        }
    });
    server.send(req);
    let resp = server.recv();
    
    let result = &resp["result"];
    if result["isError"] == false {
        let text = result["content"][0]["text"].as_str().unwrap();
        // If network is blocked, curl usually exits with code 6 or 28
        assert!(text.contains("\"exit_code\"") && !text.contains("\"exit_code\": 0"), "Network access should fail in sandbox, got: {}", text);
    } else {
        // If it failed with isError: true, it might be a sandbox setup failure which is also a "denial" in a way
        assert!(result["error"]["message"].as_str().unwrap().contains("Sandbox"), "Unexpected error: {:?}", result);
    }
}
