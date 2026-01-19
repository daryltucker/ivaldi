use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::io::AsyncReadExt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for execution safety
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SafetyConfig {
    pub isolation_mode: IsolationMode,
    #[serde(default)]
    pub ro_bind_root: bool,
    #[serde(default)]
    pub network_isolation: bool,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            isolation_mode: IsolationMode::None,
            ro_bind_root: false,
            network_isolation: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IsolationMode {
    None,
    Bubblewrap,
    // Docker (Future)
}

/// A Guard that ensures the child process is killed and reaped when dropped.
/// This enforces "You own what you start".
pub struct ProcessGuard {
    child: Option<Child>, // Option so we can take it out if we want to wait manually
}

impl ProcessGuard {
    pub fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    /// Access the inner child reference
    pub fn inner(&mut self) -> &mut Child {
        self.child.as_mut().unwrap()
    }

    /// Take the child, consuming the guard (disable auto-kill on drop)
    /// Use this if you want to allow the process to outlive the scope or handle cleanup manually.
    pub fn take(mut self) -> Child {
        self.child.take().unwrap()
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // We use start_kill (if available in newer tokio) or just kill()
            // Since this is sync drop, we can't await. We have to spawn a reaper or accept best effort synchronous kill?
            // std::process::Child::kill is sync. tokio::process::Child::start_kill is async-ish.
            
            // Tokio Drop behavior:
            // Tokio children are reaped by the runtime automatically eventually if dropped.
            // But we want to be SURE it stops running NOW.
            
            let _ = child.start_kill(); // Non-blocking signal
            // We can't await the wait() here in Drop.
            // However, tokio's runtime will handle the reaping of the zombie eventually.
            // The critical part is sending the kill signal to stop execution.
        }
    }
}

pub struct CommandRunner {
    config: SafetyConfig,
}

impl CommandRunner {
    pub fn new(config: SafetyConfig) -> Self {
        Self { config }
    }

    pub fn build(&self, program: &str, args: &[String], cwd: &Path) -> Command {
        match self.config.isolation_mode {
            IsolationMode::None => {
                let mut cmd = Command::new(program);
                cmd.args(args);
                cmd.current_dir(cwd);
                cmd
            },
            IsolationMode::Bubblewrap => {
                let mut cmd = Command::new("bwrap");
                
                // Base isolation
                if self.config.ro_bind_root {
                    cmd.arg("--ro-bind").arg("/").arg("/");
                } else {
                    cmd.arg("--bind").arg("/").arg("/");
                }
                
                cmd.arg("--dev").arg("/dev");
                cmd.arg("--proc").arg("/proc");
                
                // Network
                if self.config.network_isolation {
                    cmd.arg("--unshare-net");
                }
                
                // Workspace bind (Read-Write)
                // We map the cwd to itself so paths stay consistent
                // Note: If ro_bind_root is true, this override makes this specific path RW
                cmd.arg("--bind").arg(cwd).arg(cwd);
                cmd.current_dir(cwd);

                // Command
                cmd.arg(program);
                cmd.args(args);
                
                cmd
            }
        }
    }

    /// Execute and return stdout/stderr (Wait for completion)
    pub async fn run_capture(&self, program: &str, args: &[String], cwd: &Path, timeout_ms: u64) -> Result<(String, String, i32), anyhow::Error> {
        let mut cmd = self.build(program, args, cwd);
        
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true); // Tokio native feature, basically our ProcessGuard built-in!

        let mut child = cmd.spawn()?;
        let _child_pid = child.id(); // For logging

        // RAII Guard (Using tokio's kill_on_drop(true) on the Command is effectively the same, 
        // but let's wrap it if we did manual logic. Here usage of kill_on_drop on Command is sufficient/Better)
        
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Capture stdout/stderr concurrently to avoid deadlocks
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        let stdout_fut = async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            reader.read_to_end(&mut stdout_buf).await?;
            Ok::<Vec<u8>, std::io::Error>(stdout_buf)
        };

        let stderr_fut = async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            reader.read_to_end(&mut stderr_buf).await?;
            Ok::<Vec<u8>, std::io::Error>(stderr_buf)
        };
        
        let wait_fut = async move {
            child.wait().await
        };

        // Enforce timeout on the whole operation
        let work_fut = async {
            let (stdout_res, stderr_res, status_res) = tokio::try_join!(stdout_fut, stderr_fut, wait_fut)?;
            Ok::<(Vec<u8>, Vec<u8>, std::process::ExitStatus), anyhow::Error>((stdout_res, stderr_res, status_res))
        };

        match tokio::time::timeout(tokio::time::Duration::from_millis(timeout_ms), work_fut).await {
            Ok(res) => {
                let (out_bytes, err_bytes, status) = res?;
                let out_str = String::from_utf8_lossy(&out_bytes).to_string();
                let err_str = String::from_utf8_lossy(&err_bytes).to_string();
                Ok((out_str, err_str, status.code().unwrap_or(-1)))
            },
            Err(_) => {
                // Timeout happened. 
                // Since we set kill_on_drop(true), checking out of this scope will kill the child.
                 Err(anyhow::anyhow!("Execution timed out after {}ms", timeout_ms))
            }
        }
    }
}
