use ivaldi_core::{IvaldiResponse, ResponseStatus, AdvisoryLevel, AdvisorySource};
use serde::Serialize;
use colored::*;

pub fn print_response<T: Serialize>(response: &IvaldiResponse<T>, json_mode: bool) {
    if json_mode {
        let json = serde_json::to_string_pretty(response).unwrap_or_else(|e| {
            format!("{{\"status\": \"error\", \"error\": {{ \"code\": \"json_error\", \"message\": \"{}\" }}}}", e)
        });
        println!("{}", json);
    } else {
        print_human(response);
    }
}

fn print_human<T: Serialize>(response: &IvaldiResponse<T>) {
    // 1. Result
    match response.status {
        ResponseStatus::Success => {
             // For generic T, we can't easily pretty-print without knowing type.
             // But valid T in our case are Vec<FileMatch>, FileContent, Vec<DirEntry>.
             // We'd ideally need a specific printer or a trait `HumanDisplay`.
             // For now, let's dump the result as JSON-ish or implement specific handlers in main.rs calling this helper?
             // Actually, output.rs should probably be generic or take a formatter closure?
             // Simpler: print_response just handles the envelope (Advisories/Errors).
             // The result printing depends on the command.
             // Let's refactor: main.rs prints result, output.rs helper prints advisories/errors.
             // No, let's make this print the envelope.
        },
        ResponseStatus::Warning => {
            println!("{}", "⚠ Operation completed with warnings.".yellow());
        },
        ResponseStatus::Error => {
            if let Some(ref err) = response.error {
                 println!("{} {}: {}", "✖".red(), err.code.bold(), err.message);
                 if let Some(ref hint) = err.hint {
                     println!("  {} {}", "💡".blue(), hint);
                 }
            } else {
                 println!("{}", "✖ Unknown error occurred.".red());
            }
        }
    }

    // 2. Advisories (The Third Channel)
    if !response.advisory.is_empty() {
        println!("\n{}", "--- Advisories ---".dimmed());
        for adv in &response.advisory {
            let prefix = match adv.level {
                AdvisoryLevel::Info => "ℹ".blue(),
                AdvisoryLevel::Warn => "⚠".yellow(),
                AdvisoryLevel::Suggest => "💡".green(),
            };
            let source = match adv.source {
                AdvisorySource::Tool => "Tool".dimmed(),
                AdvisorySource::Server => "Server".magenta(),
                AdvisorySource::Adt => "ADT".cyan(),
            };
            
            println!("{} [{}] {}", prefix, source, adv.content);
            if let Some(ref action) = adv.action {
                println!("  {} {}", "→".dimmed(), action.italic());
            }
        }
    }
}

// Helper to print specific types generically?
// Maybe just expose `print_envelope` and let main handle result?
// Or `print_json` and `print_human_find_results`.

pub fn print_find_results(response: &IvaldiResponse<Vec<ivaldi_core::navigate::FileMatch>>, json: bool) {
    if json { 
        print_response(response, true); 
        return; 
    }
    
    if let Some(matches) = &response.result {
        if matches.is_empty() {
             println!("No matches found.");
        } else {
             for m in matches {
                 let size = human_bytes(m.size);
                 let icon = if m.is_dir { "📁" } else { "📄" };
                 println!("{} {:<10} {}", icon, size.dimmed(), m.path.display());
             }
             println!("\nFound {} results.", matches.len());
        }
    }
    print_human(response); // Print advisories/errors
}

pub fn print_read_result(response: &IvaldiResponse<ivaldi_core::observe::FileContent>, json: bool) {
    if json { 
        print_response(response, true); 
        return; 
    }
    
    if let Some(content) = &response.result {
        println!("{}", "--- Content ---".dimmed());
        println!("{}", content.content);
        println!("{}", "---".dimmed());
        println!("Lines: {}/{} (Truncated: {})", 
            content.info.lines_returned, 
            content.info.lines_total, 
            content.info.truncated
        );
    }
    print_human(response);
}

pub fn print_list_results(response: &IvaldiResponse<Vec<ivaldi_core::list::DirEntry>>, json: bool) {
    if json { 
        print_response(response, true); 
        return; 
    }

    if let Some(entries) = &response.result {
        println!("{:<4} {:<10} Name", "Type", "Size");
        println!("{}", "-".repeat(40).dimmed());
        for e in entries {
            let icon = if e.is_dir { "d" } else if e.is_symlink { "l" } else { "-" };
            let size = human_bytes(e.size);
            println!("{:<4} {:<10} {}", icon, size, e.name);
        }
        println!("\n{} items.", entries.len());
    }
    print_human(response);
}


pub fn print_write_result(response: &IvaldiResponse<std::path::PathBuf>, json: bool) {
    if json { 
        print_response(response, true); 
        return; 
    }

    if let Some(path) = &response.result {
         // If we had a warning about sidecar, it will be in advisories (printed by print_human)
         println!("{} Wrote to: {}", "✓".green(), path.display());
    }
    print_human(response);
}

fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 { return format!("{} B", bytes); }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 { return format!("{:.1} KB", kb); }
    let mb = kb / 1024.0;
    format!("{:.1} MB", mb)
}
