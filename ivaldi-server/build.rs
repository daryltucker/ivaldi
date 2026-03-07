use std::fs;
use std::path::Path;
use schemars::schema_for;
use serde_json::json;
use vecq::{parse_file, convert_to_json, query_json, FileType}; // Added vecq imports

use ivaldi_core::navigate::FindFilesArgs;
use ivaldi_core::observe::{ReadFileArgs, ReadFilesArgs, AnalyzeDirArgs, AnalyzeFileArgs, SearchCodeArgs, GitReadArgs, ReadSyslogsArgs};
use ivaldi_core::list::ListDirArgs;
use ivaldi_core::mutate::{WriteFileArgs, EditFileArgs, EditFilesArgs};
use ivaldi_core::undo::UndoArgs;
use ivaldi_core::session::types::{SessionInitArgs, SessionListArgs, SessionGetArgs, SessionUpdateArgs};

// Define run_command schema manually since it's in ivaldi_server (circular dependency)
/// Arguments for the run_command tool
///
/// **Behavior**: Executes shell commands with timeout protection and policy controls.
/// **Safety**: Subject to security policy checks. Commands are executed in isolated processes.
/// **Advisory**: Use with caution. Commands may have side effects on the filesystem.
/// **Usage**: Run shell commands like 'git status', 'npm install', or custom scripts.
#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct RunCommandArgs {
    /// The program to execute (e.g. "git", "ls")
    command: String,
    /// Arguments to pass to the program
    args: Vec<String>,
    /// Working directory (optional, defaults to project root)
    cwd: Option<String>,
    /// Timeout in milliseconds (default 5000)
    timeout_ms: Option<u64>,
}

fn main() -> anyhow::Result<()> {
    // 1. Setup paths
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    
    // 2. Generate Schemas
    let schemas = vec![
        ("find_files", schema_for!(FindFilesArgs)),
        ("read_file", schema_for!(ReadFileArgs)),
        ("read_files", schema_for!(ReadFilesArgs)),
        ("list_dir", schema_for!(ListDirArgs)),
        ("run_command", schema_for!(RunCommandArgs)),
        ("write_file", schema_for!(WriteFileArgs)),
        ("edit_file", schema_for!(EditFileArgs)),
        ("edit_files", schema_for!(EditFilesArgs)),
        ("undo", schema_for!(UndoArgs)),
        ("edit_file", schema_for!(EditFileArgs)),
        ("undo", schema_for!(UndoArgs)),
        ("analyze_dir", schema_for!(AnalyzeDirArgs)),
        ("analyze_file", schema_for!(AnalyzeFileArgs)),
        ("search_code", schema_for!(SearchCodeArgs)),
        ("git_read", schema_for!(GitReadArgs)),
        ("read_syslogs", schema_for!(ReadSyslogsArgs)),
        // Session Management
        ("session_init", schema_for!(SessionInitArgs)),
        ("session_list", schema_for!(SessionListArgs)),
        ("session_get", schema_for!(SessionGetArgs)),
        ("session_update", schema_for!(SessionUpdateArgs)),
    ];

    let config_schema = schema_for!(ivaldi_core::config::GlobalConfig);

    // 3. Combine into Tool Objects
    let mut tools_json = Vec::new();
    for (name, schema) in schemas {
        // Extract description from schema (doc comments)
        let description = schema.schema.metadata.as_ref()
            .and_then(|m| m.description.as_ref())
            .cloned()
            .unwrap_or_else(|| format!("No description found for {}", name));

        let mut tool = json!({
            "name": name,
            "description": description,
            "inputSchema": schema
        });
        
        // Normalize schema for universal compatibility (removes extended JSON Schema fields)
        normalize_schema(&mut tool["inputSchema"]);
        tools_json.push(tool);
    }

    // 4. Construct Final JSON (Agent Runtime)
    let manual_json = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "mcp-2024-11-05",
        "generated_at": "build-time",
        "tools": tools_json,
    });

    // 5. Generate CONFIGURATION.md
    let config_docs = generate_config_docs(&config_schema)?;
    let config_path = Path::new(&manifest_dir).join("../docs/CONFIGURATION.md");
    
    // Ensure parent directory exists (critical for Docker builds where ../docs might not be copied)
    if let Some(parent) = config_path.parent() && !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(&config_path, config_docs)?;

    // 6. Generate MAN_AGENT.json in docs/
    let man_agent_path = Path::new(&manifest_dir).join("../docs/MAN_AGENT.json");
    
    // Ensure parent directory exists (though likely handled by previous block)
    if let Some(parent) = man_agent_path.parent() && !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(&man_agent_path, serde_json::to_string_pretty(&manual_json)?)?;

    // 7. Write runtime manual (OUT_DIR and project root for embedding)
    let out_dir = std::env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("runtime_manual.json");
    fs::write(&dest_path, serde_json::to_string_pretty(&manual_json)?)?;
    let _ = fs::write(Path::new(&manifest_dir).join("runtime_manual.json"), serde_json::to_string_pretty(&manual_json)?);

    // 8. Generate IDE Metadata Schema (ivaldi.schema.json)
    let meta_schema = schema_for!(ivaldi_core::meta::ToolCallContext);
    let meta_schema_path = Path::new(&manifest_dir).join("../docs/ivaldi.schema.json");
    
    // Ensure parent directory exists
    if let Some(parent) = meta_schema_path.parent() && !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(&meta_schema_path, serde_json::to_string_pretty(&meta_schema)?)?;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_schemas.rs");
    Ok(())
}

fn generate_config_docs(schema: &schemars::schema::RootSchema) -> anyhow::Result<String> {
    let mut docs = String::from("## GLOBAL CONFIGURATION\n\n");
    docs.push_str("These parameters change the running state of the system and can be set via CLI flags or namespaced environment variables.\n\n");
    
    // Server-specific CLI options (auto-generated from vecq parsing of src/cli.rs)
    docs.push_str("### Server Options\n\n");
    docs.push_str("| Option | Environment Variable | CLI Flag | Description | Default |\n");
    docs.push_str("|--------|----------------------|----------|-------------|---------|\n");
    
    // Use vecq library with custom jq filter to extract Args struct metadata at compile-time
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let cli_path = Path::new(&manifest_dir).join("src/cli.rs");

    // Read source and filter files
    let cli_content = fs::read_to_string(&cli_path).unwrap_or_default();
    
    // NOTE: This filter is a copy of `ivaldi-server/functions/extract_cli_args.jq`.
    // If we ever change that file, we must update this fixture.
    // We embed it here to avoid build-time file dependency issues in Docker.
    const CLI_ARGS_FILTER: &str = r###"
.structs[]
| select(.name == "Args")
| .content
| split("\n")
| . as $lines
| reduce range(0; length) as $i (
    [];
    if $lines[$i] | test("^\\s*///") then
      # Found a doc comment
      . + [{
        description: ($lines[$i] | gsub("^\\s*///\\s*"; "")),
        line: $i
      }]
    else
      .
    end
  )
| map(
    . as $doc
    | $lines[$doc.line + 1] as $attr_line
    | $lines[$doc.line + 2] as $field_line
    | if ($attr_line | test("#\\[arg\\(")) then
        {
          description: $doc.description,
          name: ($field_line | gsub("^\\s*pub\\s+"; "") | gsub(":.*$"; "") | gsub("\\s+"; "")),
          env: (
            if ($attr_line | test("env\\s*=\\s*\"")) then
              $attr_line | gsub(".*env\\s*=\\s*\""; "") | gsub("\".*"; "")
            else
              ""
            end
          ),
          flag: (
            if ($attr_line | test("long")) then
              "--" + ($field_line | gsub("^\\s*pub\\s+"; "") | gsub(":.*$"; "") | gsub("\\s+"; ""))
            else
              ""
            end
          ),
          default: "None"
        }
      else
        empty
      end
  )
"###;

    if !cli_content.is_empty() {
        // Create a local tokio runtime to run async vecq operations
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let vecq_result = rt.block_on(async {
            let parsed = parse_file(&cli_content, FileType::Rust).await?;
            let json = convert_to_json(parsed)?;
            query_json(&json, CLI_ARGS_FILTER)
        });

        match vecq_result {
            Ok(results) => {
                // Determine if results is a single array (usual case if filter outputs an array) or multiple values
                // The filter extract_cli_args.jq likely outputs an array of objects.
                // query_json returns Vec<Value>. If the filter returns [Object, Object], results will be [Object, Object].
                // However, if the filter itself outputs an Array [..], results might be [Array].
                // We should flatten if needed or iterate.
                
                // Let's assume the filter produces a stream of objects or a single array of objects.
                // Based on previous code: serde_json::from_str::<Vec<serde_json::Value>>(&json_output)
                // This implies the CLI version returned a JSON array.
                // jaq/vecq query_json returns all outputs of the filter.
                
                // Inspect the first result to see if it's an array
                let items = if !results.is_empty() && results[0].is_array() {
                    results[0].as_array().unwrap().clone()
                } else {
                    results
                };

                for arg in items {
                    let name = arg.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let env = arg.get("env").and_then(|v| v.as_str()).unwrap_or("");
                    let flag = arg.get("flag").and_then(|v| v.as_str()).unwrap_or("").replace("_", "-");
                    let description = arg.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let default = arg.get("default").and_then(|v| v.as_str()).unwrap_or("None");
                    
                    if !name.is_empty() {
                        docs.push_str(&format!(
                            "| `{}` | `{}` | `{}` | {} | {} |\n",
                            name, env, flag, description, default
                        ));
                    }
                }
            },
            Err(e) => {
                eprintln!("Warning: vecq library call failed: {}", e);
                 // Fallback
                docs.push_str("| `conversation_id` | `IVALDI_CONVERSATION_ID` | `--conversation-id` | Conversation ID for naked/stdio drivers | None |\n");
                docs.push_str("| `conversation_mode` | `IVALDI_CONVERSATION_MODE` | `--conversation-mode` | Conversation mode | None |\n");
            }
        }
    } else {
        eprintln!("Warning: Could not read cli.rs");
    }
    
    docs.push('\n');
    
    // Core config options (from GlobalConfig schema)
    docs.push_str("### Core Options\n\n");
    docs.push_str("| Option | Environment Variable | CLI Flag | Description | Default |\n");
    docs.push_str("|--------|----------------------|----------|-------------|---------|\n");

    if let Some(obj) = &schema.schema.object {
        for (name, subschema) in &obj.properties {
            let metadata = subschema.clone().into_object().metadata;
            let description = metadata.as_ref().and_then(|m| m.description.as_ref()).map(|s| s.as_str()).unwrap_or("");
            
            // Extract CLI/ENV from description
            let env_var = description.lines().find(|l| l.contains("ENV:")).map(|l| l.split("ENV:").last().unwrap().trim()).unwrap_or("");
            let cli_flag = description.lines().find(|l| l.contains("CLI:")).map(|l| l.split("CLI:").last().unwrap().trim()).unwrap_or("");
            let clean_desc = description.lines().next().unwrap_or("");

            docs.push_str(&format!("| `{}` | `{}` | `{}` | {} | `false` |\n", name, env_var, cli_flag, clean_desc));
        }
    }

    Ok(docs)
}

// Include the schema normalization logic
include!("build_schemas.rs");
