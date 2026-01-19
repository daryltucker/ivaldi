#![allow(dead_code)]
#![allow(unused_imports)]
use std::process::{Command, Child, Stdio};
use std::time::Duration;
use std::thread;
use tempfile::TempDir;
use std::net::TcpStream;

/// Fixed test config path for hermetic testing
pub fn test_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("tests/fixtures/config.toml")
}

/// Spawns the ivaldi-server in HTTP mode on a specified port
pub fn spawn_http_server(port: u16, temp_dir: &TempDir) -> (Child, u16) {
    // Build the server binary first
    let status = Command::new("cargo")
        .args(&["build", "--bin", "ivaldi-server"])
        .status()
        .expect("Failed to build server");
    assert!(status.success(), "Server binary must build");
    
    let server_bin = env!("CARGO_BIN_EXE_ivaldi-server");
    
    let mut child = Command::new(server_bin)
        .env("IVALDI_CONFIG", test_config_path())
        .env("IVALDI_LOG", "info") // Need info to see the port
        .current_dir(temp_dir.path())
        .args(&["--transport", "http", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped()) 
        .spawn()
        .expect("Failed to spawn HTTP server");
        
    // If port 0, Read stderr to find the actual port
    let actual_port = if port == 0 {
        let stderr = child.stderr.take().expect("Failed to capture stderr");
        let mut reader = std::io::BufReader::new(stderr);
        let mut line = String::new();
        let mut found_port = 0;
        
        // Read lines until we find the port or timeout/EOF
        // We need to put stderr back or handle it. 
        // We can't put it back easily. So we'll have to consume it in a thread or just grep it.
        // For simplicity, let's just read line by line.
        
        use std::io::BufRead;
        while reader.read_line(&mut line).unwrap() > 0 {
            // println!("Server Log: {}", line.trim()); // Debug
            if line.contains("Listening on") {
                // Format: ... Listening on 0.0.0.0:12345
                if let Some(addr_str) = line.trim().split_whitespace().last() {
                    if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
                        found_port = addr.port();
                        break;
                    }
                }
            }
            line.clear();
        }
        
        if found_port == 0 {
            panic!("Failed to find bound port in server logs");
        }
        found_port
    } else {
        port
    };
    
    (child, actual_port)
}

/// Wait for server to be ready (port open) with timeout
pub fn wait_for_server(port: u16, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let addr = format!("127.0.0.1:{}", port);
    
    while start.elapsed().as_millis() < timeout_ms as u128 {
        if TcpStream::connect(&addr).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

/// RAII Wrapper for the Server Process (The "Real Suite")
pub struct TestServer {
    process: Child,
    pub port: u16,
    pub temp_dir: TempDir, 
}

impl TestServer {
    /// Start a new server instance with its own temp directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let (process, port) = spawn_http_server(0, &temp_dir);
        
        // Wait for readiness
        if !wait_for_server(port, 2000) {
            panic!("Server failed to start on port {}", port);
        }

        Self {
            process,
            port,
            temp_dir,
        }
    }
    
    /// Get the MCP endpoint URL
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }
    
    /// Get the project root path
    pub fn root(&self) -> std::path::PathBuf {
        self.temp_dir.path().to_path_buf()
    }
    
    /// Create a pre-configured HTTP client
    pub fn client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    /// Helper: Initialize the session
    pub async fn initialize(&self, client: &reqwest::Client) {
        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        });
        let resp = client.post(&self.url())
            .json(&init_req)
            .send()
            .await
            .expect("Failed to send init");
        assert!(resp.status().is_success(), "Initialize failed");
    }

    /// Helper: Call a tool
    pub async fn call_tool(&self, client: &reqwest::Client, name: &str, args: serde_json::Value) -> serde_json::Value {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1, // ID doesn't matter much for these tests
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args
            }
        });
        
        let resp = client.post(&self.url())
            .json(&req)
            .send()
            .await
            .expect("Failed to call tool");
            
        assert!(resp.status().is_success(), "Tool call network fail");
        let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
        assert!(body.get("error").is_none(), "Tool returned error: {:?}", body);
        body
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // "Take pride" -> Clean up your mess
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
