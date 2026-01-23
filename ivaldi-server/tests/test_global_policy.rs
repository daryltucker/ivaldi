use std::process::{Command, Stdio};
use std::fs;
use std::io::{Write, BufRead, BufReader};
use tempfile::tempdir;
use serde_json::json;

#[test]
fn test_global_policy_enforcement() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let root = temp_dir.path().join("project");
    let fake_home = temp_dir.path().join("home");
    let global_policy_dir = fake_home.join(".config/ivaldi/policies");

    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&global_policy_dir).unwrap();

    let server_bin = env!("CARGO_BIN_EXE_ivaldi-server");

    // Setup Global Policy: FORBID echo
    fs::write(global_policy_dir.join("global.cedar"), "forbid(principal, action == Action::\"exec\", resource == Command::\"echo\");").unwrap();

    let mut child = Command::new(server_bin)
        .current_dir(&root)
        .env("HOME", &fake_home)
        .env_remove("XDG_CONFIG_HOME")
        .arg("--transport").arg("stdio")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    stdin.write_all(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string().as_bytes()).unwrap();
    stdin.write_all(b"\n").unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    // Call echo -> Should be DENIED by Global Policy
    stdin.write_all(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "run_command", "arguments": {"command": "echo", "args": ["global test"]}}}).to_string().as_bytes()).unwrap();
    stdin.write_all(b"\n").unwrap();
    
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    
    child.kill().unwrap();
    assert_eq!(resp["result"]["isError"], true, "Should be DENIED by Global Policy. Log: {}", line);
}
