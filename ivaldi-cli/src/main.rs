//! # ivaldi CLI
//!
//! Human-facing command-line interface for ivaldi operations.
//!
//! ## PURPOSE
//!
//! This binary provides a CLI for humans to interact with ivaldi.
//! Agents should use the MCP server (`ivaldi-server`) instead.
//!
//! ## COMMANDS (Planned)
//!
//! ```text
//! ivaldi find <pattern>      # Find files matching pattern
//! ivaldi read <file>         # Read file with optional line range
//! ivaldi write <file>        # Write file (with pre-flight checks)
//! ivaldi edit <file>         # AST-based editing
//! ivaldi undo                # Undo last operation
//! ivaldi history             # Show operation journal
//! ```
//!
//! ## OUTPUT MODES
//!
//! - Default: Human-readable with colors (when TTY)
//! - `--json`: Machine-readable JSON (IvaldiResponse format)
//!
//! ## ADVISORY DISPLAY
//!
//! When operations include advisory messages, they are displayed:
//! - Info: dim text after result
//! - Warn: yellow prefixed with ⚠
//! - Suggest: blue prefixed with 💡
//!
//! ## PHILOSOPHY
//!
//! The CLI is a thin translation layer:
//! 1. Parse arguments with clap
//! 2. Call ivaldi-core operations
//! 3. Format output for humans
//!
//! All logic lives in ivaldi-core. Never bypass.

mod output;
use output::{print_find_results, print_read_result, print_list_results, print_write_result}; 

use clap::Parser;
use ivaldi_core::navigate::{FsNavigator, Navigator, FindFilesArgs};
use ivaldi_core::observe::{FsObserver, Observer, ReadFileArgs};
use ivaldi_core::list::{FsLister, Lister, ListDirArgs};
use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs};
use ivaldi_core::undo::{Journal, Undoer};
use ivaldi_core::lifecycle::project_root::find_project_root;
use std::io::{Read, self};

mod args;
use args::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    
    // Lifecycle: Find Project Root
    let start_path = std::env::current_dir()?;
    let root = find_project_root(&start_path);
    // Tracing info? eprintln!("Project Root: {:?}", root);

    match cli.command {
        Some(Commands::Find { dir, pattern, depth, no_ignore }) => {
            let args = FindFilesArgs {
                path: dir,
                pattern,
                max_depth: depth.unwrap_or(5),
                max_entries: 100,
                timeout_ms: 5000,
                enable_gitignore: !no_ignore,
                respect_aiignore: true,
                respect_agentignore: true,
            };
            let response = FsNavigator::find_files(args);
            print_find_results(&response, cli.json);
        }
        Some(Commands::Read { path, from, to, force, query, grep, context }) => {
             let args = ReadFileArgs {
                 path,
                 from_line: from,
                 to_line: to,
                 force,
                 query,
                 grep,
                 context_lines: context,
             };
             let response = FsObserver::read_file(args);
             print_read_result(&response, cli.json);
        }
        Some(Commands::List { path, all }) => {
             let args = ListDirArgs {
                 path,
                 sort: true,
                 show_hidden: all,
                 enable_gitignore: false,
                 respect_aiignore: true,
                 respect_agentignore: true,
             };
             let response = FsLister::list_dir(args);
             print_list_results(&response, cli.json);
        }
        Some(Commands::Write { path, content, force, append }) => {

            // 1. Resolve content (Arg or Stdin)
            let text_content = match content {
                Some(s) => s,
                None => {
                    let mut buffer = String::new();
                    io::stdin().read_to_string(&mut buffer)?;
                    buffer
                }
            };

            // 2. Initialize Journal from Root
            let journal_path = root.join(".ivaldi/journal.jsonl");
            let journal = match Journal::open(&journal_path) {
                Ok(j) => j,
                Err(e) => {
                     eprintln!("Failed to open journal at {:?}: {}", journal_path, e);
                     return Ok(()); 
                }
            };

            // 3. Execute
            let args = WriteFileArgs {
                path, 
                content: text_content,
                overwrite: force,
                append,
            };
            
            // Pass root to Mutator
            let response = Mutator::write_file(&root, args, &journal);
            print_write_result(&response, cli.json);
        }
        Some(Commands::Edit { path, query, grep, from, to, replacement }) => {

            let journal_path = root.join(".ivaldi/journal.jsonl");
            let journal = match Journal::open(&journal_path) {
                Ok(j) => j,
                Err(e) => {
                     eprintln!("Failed to open journal at {:?}: {}", journal_path, e);
                     return Ok(());
                }
            };

            let args = EditFileArgs {
                path,
                replacement,
                query,
                grep,
                from_line: from,
                to_line: to,
            };
            
            // Edit is async
            let rt = tokio::runtime::Runtime::new()?;
            let response = rt.block_on(async {
                Mutator::edit_file(&root, args, &journal).await
            });
            
            print_write_result(&response, cli.json); 
        }
        Some(Commands::Undo) => {
             let journal_path = root.join(".ivaldi/journal.jsonl");
             let journal = match Journal::open(&journal_path) {
                Ok(j) => j,
                Err(e) => {
                     eprintln!("Failed to open journal at {:?}: {}", journal_path, e);
                     return Ok(());
                }
            };

            let response = Undoer::undo_last(&root, &journal);
            print_write_result(&response, cli.json);
        }
        Some(Commands::Status) => {
            println!("ivaldi v{}", env!("CARGO_PKG_VERSION"));
            println!("Status: Operational (v{})", env!("CARGO_PKG_VERSION"));
            println!("Modules: Navigation, Observation, Sensors, Mutation (Undo)");
            println!("Project Root: {:?}", root);
            if let Some(config) = &cli.config {
                println!("Config File: {}", config);
            }
            if let Some(key) = &cli.api_key {
                let masked = if key.len() > 8 {
                    format!("{}...{}", &key[0..4], &key[key.len()-4..])
                } else {
                    "********".to_string()
                };
                println!("API Key: {}", masked);
            }
        }
        None => {
             use clap::CommandFactory;
             Cli::command().print_help()?;
        }
    }
    
    Ok(())
}