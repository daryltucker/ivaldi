use std::process::{Command, Child, Stdio, ChildStdin, ChildStdout};
use std::io::{Write, BufRead, BufReader};
use std::fs;
use tempfile::TempDir;
use serde_json::json;

/// RAII Wrapper for Stdio Server Process
pub struct StdioTestServer {
    pub process: Child,
    pub stdin: Option<ChildStdin>, // Option to allow taking it
    pub reader: Option<BufReader<ChildStdout>>, // Buffered reader for stdout
    pub temp_dir: TempDir,
    pub root: std::path::PathBuf,
}

impl StdioTestServer {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let root = temp_dir.path().join("project");
        fs::create_dir_all(&root).expect("Failed to create project root");

        // Build server binary
        let status = Command::new("cargo")
            .args(&["build", "--bin", "ivaldi-server"])
            .status()
            .expect("Failed to build server");
        assert!(status.success(), "Server binary build failed");

        let server_bin = env!("CARGO_BIN_EXE_ivaldi-server");
        
        // Spawn server
        let mut child = Command::new(server_bin)
            .env("IVALDI_CONFIG", temp_dir.path().join("config"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Let stderr flow through for debugging
            .spawn()
            .expect("Failed to spawn server");

        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(stdout);

        Self {
            process: child,
            stdin: Some(stdin),
            reader: Some(reader),
            temp_dir,
            root,
        }
    }

    /// Send a JSON-RPC request
    pub fn send(&mut self, req: serde_json::Value) {
        let req_str = req.to_string();
        if let Some(stdin) = self.stdin.as_mut() {
            stdin.write_all(req_str.as_bytes()).expect("Failed to write to stdin");
            stdin.write_all(b"\n").expect("Failed to write newline to stdin");
            stdin.flush().expect("Failed to flush stdin");
        } else {
            panic!("Stdin not available");
        }
    }

    /// Read a JSON-RPC response
    pub fn recv(&mut self) -> serde_json::Value {
        let mut line = String::new();
        if let Some(reader) = self.reader.as_mut() {
            let bytes = reader.read_line(&mut line).expect("Failed to read line from stdout");
            if bytes == 0 {
                panic!("Server closed stdout unexpectedly");
            }
            serde_json::from_str(&line).expect("Failed to parse JSON response")
        } else {
            panic!("Stdout reader not available");
        }
    }

    /// Helper to initialize default session
    pub fn initialize(&mut self) {
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        });
        self.send(init_req);
        let resp = self.recv();
        assert!(resp.get("error").is_none(), "Initialize failed: {:?}", resp);
    }
}

impl Drop for StdioTestServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
