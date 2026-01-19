use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use ivaldi_core::response::IvaldiResponse;
use ivaldi_core::advisory::AdvisoryMessage;
use ivaldi_core::execution::CommandRunner;
use crate::state::ServerState;

/// Arguments for the `run_command` tool
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RunCommandArgs {
    /// The program to execute (e.g. "git", "ls")
    pub command: String,
    /// Arguments to pass to the program
    pub args: Vec<String>,
    /// Working directory (optional, defaults to project root)
    pub cwd: Option<String>,
    /// Timeout in milliseconds (default 5000)
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema, Default)]
pub struct RunCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn run_command(args: RunCommandArgs, state: &ServerState) -> anyhow::Result<IvaldiResponse<RunCommandResult>> {
    // 1. Resolve CWD
    let project_root = state.get_session().map(|s| s.root).unwrap_or_else(|| std::env::current_dir().unwrap());
    let cwd = if let Some(p) = args.cwd {
        project_root.join(p)
    } else {
        project_root.clone()
    };

    // 2. Policy Check
    // Principal: "Agent" (static for now)
    // Action: "exec" (could differentiate read/write later)
    // Resource: The command name itself
    let allowed = state.policy_engine().check("Entity::\"Agent\"", "Action::\"exec\"", &format!("Command::\"{}\"", args.command))
        .map_err(|e| anyhow::anyhow!("Policy check error: {}", e))?;

    if !allowed {
        return Ok(IvaldiResponse::error(
            "-32003", // Custom auth error code
            format!("Permission denied: Execution of command '{}' is restricted by security policy.", args.command)
        ).with_context(serde_json::json!({ "policy": "cedar-denied" })));
    }

    // 3. Execution
    let config = state.config().safety.clone();
    let runner = CommandRunner::new(config);
    let timeout = args.timeout_ms.unwrap_or(5000);

    // Run with timeout protection via CommandRunner which uses tokio::timeout
    match runner.run_capture(&args.command, &args.args, &cwd, timeout).await {
        Ok((stdout, stderr, code)) => {
            let result = RunCommandResult {
                stdout,
                stderr,
                exit_code: code,
            };
            
            // Generate advisory if stderr has content
            let mut advisories = Vec::new();
            if !result.stderr.is_empty() {
                advisories.push(AdvisoryMessage::tool_warn(
                    serde_json::json!({
                        "stderr": result.stderr, 
                        "message": "Command wrote to stderr" 
                    })
                ));
            }

            Ok(IvaldiResponse::success(result).with_advisories(advisories))
        },
        Err(e) => {
            // Could be timeout or spawn fail
            Ok(IvaldiResponse::error("execution_failed", format!("Command execution failed: {}", e)))
        }
    }
}
