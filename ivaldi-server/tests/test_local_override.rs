use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::tempdir;

#[test]
fn test_local_policy_overrides_global() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let root = temp_dir.path().join("project");
    let fake_home = temp_dir.path().join("home");
    let global_policy_dir = fake_home.join(".config/ivaldi/policies");
    let local_policy_dir = root.join(".ivaldi/policies");

    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&global_policy_dir).unwrap();
    fs::create_dir_all(&local_policy_dir).unwrap();

    let server_bin = env!("CARGO_BIN_EXE_ivaldi-server");

    // Global: FORBID echo
    fs::write(
        global_policy_dir.join("global.cedar"),
        "forbid(principal, action == Action::\"exec\", resource == Command::\"echo\");",
    )
    .unwrap();
    // Local: FORBID touch (Implicitly allows echo because local override means we DON'T load global)
    fs::write(
        local_policy_dir.join("local.cedar"),
        "forbid(principal, action == Action::\"exec\", resource == Command::\"touch\");",
    )
    .unwrap();

    let mut child = Command::new(server_bin)
        .current_dir(&root)
        .env("HOME", &fake_home)
        .env_remove("XDG_CONFIG_HOME")
        .arg("--transport")
        .arg("stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    stdin
        .write_all(
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    stdin.write_all(b"\n").unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    // Call echo -> Should be ALLOWED because Global is ignored when Local exists
    stdin.write_all(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "run_command", "arguments": {"command": "echo", "args": ["local override"]}}}).to_string().as_bytes()).unwrap();
    stdin.write_all(b"\n").unwrap();

    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    child.kill().unwrap();
    // Check that the response is successful (has result, no error)
    assert!(
        resp.get("result").is_some(),
        "Response should have result field. Response: {}",
        line
    );
    assert!(
        resp.get("error").is_none(),
        "Response should not have error field. Response: {}",
        line
    );
    // Check that the command output contains "local override"
    // In MCP format, the content is a JSON string in text field
    let text_content = resp["result"]["content"][0]["text"].as_str().unwrap();
    let parsed_content: serde_json::Value = serde_json::from_str(text_content).unwrap();
    let content = parsed_content["stdout"].as_str().unwrap();
    assert!(
        content.contains("local override"),
        "Output should contain 'local override'. Response: {}",
        line
    );
}
