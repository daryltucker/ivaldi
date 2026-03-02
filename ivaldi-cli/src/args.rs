use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ivaldi")]
#[command(author, version, about = "Precision file operations for AI Agents")]
pub struct Cli {
    /// Output as JSON (IvaldiResponse format)
    #[arg(long, global = true)]
    pub json: bool,

    /// API Key for authenticated services
    #[arg(long, global = true, env = "IVALDI_API_KEY")]
    pub api_key: Option<String>,

    /// Path to a custom configuration file
    #[arg(long, short = 'c', global = true, env = "IVALDI_CONFIG")]
    pub config: Option<String>,
    
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Find files matching a pattern (Radar)
    Find {
        /// Directory to search in
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// The pattern to search for (glob)
        #[arg(default_value = "*")]
        pattern: String,
        
         /// Max depth
        #[arg(long)]
        depth: Option<usize>,
        
        /// Ignore .gitignore
        #[arg(long)]
        no_ignore: bool,
    },
    
    /// Read a file (Telescope)
    Read {
        /// File path
        path: PathBuf,
        
        /// Start line (1-indexed)
        #[arg(long)]
        from: Option<usize>,
        
        /// End line (1-indexed, inclusive)
        #[arg(long)]
        to: Option<usize>,
        
        /// Force read (bypass binary/size checks)
        #[arg(long)]
        force: bool,

        /// AST node selector (vecq query)
        #[arg(long, short = 'q')]
        query: Option<String>,

        /// Grep pattern selector
        #[arg(long, short = 'g')]
        grep: Option<String>,

        /// Context lines for grep (default: 2)
        #[arg(long, short = 'C')]
        context: Option<usize>,
    },
    
    /// List directory contents (Sensors)
    List {
        /// Directory path
        #[arg(default_value = ".")]
        path: PathBuf,
        
        /// Show hidden files
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// Write/Create a file (The Hammer)
    Write {
        /// Target file path
        path: PathBuf,

        /// Content to write (if omitted, reads from stdin)
        content: Option<String>,

        /// Force overwrite (default: append to existing files)
        #[arg(long, short = 'f')]
        force: bool,

        /// Explicitly append (quieter advisory)
        #[arg(long, short = 'a')]
        append: bool,
    },


    /// Edit a file surgically (The Scalpel)
    Edit {
        /// Target file path
        path: PathBuf,

        /// AST node selector (vecq query)
        #[arg(long, short = 'q')]
        query: Option<String>,

        /// Grep pattern selector
        #[arg(long, short = 'g')]
        grep: Option<String>,

        /// Start line (1-indexed)
        #[arg(long)]
        from: Option<usize>,

        /// End line (1-indexed)
        #[arg(long)]
        to: Option<usize>,

        /// Replacement content
        #[arg(long, short = 'r')]
        replacement: String,
    },

    /// Undo the last operation (The Time Machine)
    Undo,

    /// Display version and configuration
    Status,
}
