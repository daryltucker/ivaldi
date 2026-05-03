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
    /// Timeout in milliseconds (default 30000)
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
        eprintln!("DEBUG: Policy denied access to command '{}'", args.command);
        let error_response = IvaldiResponse::error(
            "-32003", // Custom auth error code
            format!("Permission denied: Execution of command '{}' is restricted by security policy.", args.command)
        ).with_context(serde_json::json!({ "policy": "cedar-denied" }));
        eprintln!("DEBUG: Returning error response: is_error={}, content={:?}", error_response.is_error, error_response.content);
        return Ok(error_response);
    }

    // 3. Execution
    let config = state.config().safety.clone();
    let runner = CommandRunner::new(config);
    let timeout = args.timeout_ms.unwrap_or(30000);

    // Run with timeout protection via CommandRunner which uses tokio::timeout
    match runner.run_capture(&args.command, &args.args, &cwd, timeout).await {
        Ok((stdout, stderr, code)) => {
            let result = RunCommandResult {
                stdout,
                stderr,
                exit_code: code,
            };
            
            // Generate advisory if stderr has content
            // Downgrade to Info when exit_code == 0 (benign stderr like `time` output)
            let mut advisories = Vec::new();
            if !result.stderr.is_empty() {
                if result.exit_code == 0 {
                    advisories.push(AdvisoryMessage::tool_info(
                        serde_json::json!({
                            "stderr": result.stderr, 
                            "message": "Command wrote to stderr (exit 0 - informational)" 
                        })
                    ));
                } else {
                    advisories.push(AdvisoryMessage::tool_warn(
                        serde_json::json!({
                            "stderr": result.stderr, 
                            "message": "Command wrote to stderr" 
                        })
                    ));
                }
            }

            // Buffering Heuristic (Advisory channel)
            if (args.command.contains("python") || args.args.iter().any(|a| a.ends_with(".py"))) 
               && result.stdout.is_empty() 
               && result.exit_code == 0 
            {
                advisories.push(AdvisoryMessage::tool_info(serde_json::json!({
                    "suggestion": "Empty output detected from Python script. If you expected output, ensure stdout is flushed or run python with '-u' for unbuffered output."
                })));
            }

            Ok(IvaldiResponse::success(result).with_advisories(advisories))
        },
        Err(e) => {
            // Could be timeout or spawn fail
            Ok(IvaldiResponse::error("execution_failed", format!("Command execution failed: {}", e)))
        }
    }
}
