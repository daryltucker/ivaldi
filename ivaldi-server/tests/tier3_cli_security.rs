use serde_json::json;
use std::fs;
use std::collections::HashMap;

mod common;
use common::stdio::StdioTestServer;

// Helper to assert response is success
fn assert_success(resp: &serde_json::Value, expected_stdout: Option<&str>) {
    println!("Checking success for: {:?}", resp);
    let result = &resp["result"];
    assert_eq!(result["status"], "success", "Response should be success");
    
    if let Some(expected) = expected_stdout {
        let stdout = result["result"]["stdout"].as_str().unwrap();
        assert!(stdout.contains(expected), "Stdout '{}' should contain '{}'", stdout, expected);
    }
}



#[test]
fn test_cli_execution_policy_enforcement() {
    // 1. Prepare env
    let (temp_dir, root) = StdioTestServer::prepare();
    
    // 2. Setup Policy
    let policy_dir = root.join(".ivaldi").join("policies");
    fs::create_dir_all(&policy_dir).expect("Failed to create policies dir");
    
    let policy_content = r#"
        permit(
            principal == Entity::"Agent", 
            action == Action::"exec", 
            resource == Command::"echo"
        );
    "#;
    
    fs::write(policy_dir.join("test.cedar"), policy_content).expect("Failed to write policy");

    // 3. Spawn Server
    let mut server = StdioTestServer::spawn(temp_dir, root);
    server.initialize();

    // 4. Test Allowed Command (echo)
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

    // 5. Test Denied Command (ls)
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
    assert_eq!(ls_ivaldi_resp["status"], "error");
    assert_eq!(ls_ivaldi_resp["error"]["code"], "-32003");
    assert!(ls_ivaldi_resp["error"]["message"].as_str().unwrap().contains("Permission denied"));
}

#[test]
fn test_default_deny_enforcement() {
    // 1. Prepare env (No policies)
    let (temp_dir, root) = StdioTestServer::prepare();
    
    let mut server = StdioTestServer::spawn(temp_dir, root);
    server.initialize();

    // 2. Try echo (should fail now)
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
    assert_eq!(ivaldi_resp["status"], "error");
    assert_eq!(ivaldi_resp["error"]["code"], "-32003");
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
    if result["status"] == "success" {
        let inner = &result["result"];
        assert_ne!(inner["exit_code"].as_i64().unwrap(), 0, "Writing to / should fail in sandbox");
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
    if result["status"] == "success" {
        let inner = &result["result"];
        let exit_code = inner["exit_code"].as_i64().unwrap();
        assert_ne!(exit_code, 0, "Network access should fail in sandbox");
    }
}
