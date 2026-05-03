//! # .agentignore Helper
//!
//! Centralized logic for applying `.agentignore` to an `ignore::WalkBuilder`.
//! Keeps the per-tool logic consistent: per-directory `.agentignore` files
//! through the `ignore` crate, plus a global `~/.agentignore` as a baseline.
//!
//! ## Philosophy
//! `.agentignore` is a signal-to-noise filter for agent file-walking, NOT a
//! security boundary. Explicit-path tools (read_file, edit_file) never respect it.
//! Agents can bypass it with `respect_agentignore: false` per tool call.
//!
//! ## Resolution order (last wins / most specific wins):
//! 1. `~/.agentignore` — user-global defaults (lowest priority)
//! 2. `./.agentignore` — per-directory project overrides (higher priority)

use std::path::PathBuf;

/// Apply `.agentignore` filtering to a `WalkBuilder`, if `respect` is true.
///
/// Adds:
/// - Global `~/.agentignore` (user home) as a pre-loaded ignore file;
///   patterns are relative to the walk root.
/// - Per-directory `.agentignore` files (via `add_custom_ignore_filename`),
///   searched at every level during the walk.
pub fn apply(builder: &mut ignore::WalkBuilder, respect: bool) {
    if !respect {
        return;
    }

    // 1. Global fallback: ~/.agentignore (loaded once at walk root)
    if let Some(home) = home_dir() {
        let global_path = home.join(".agentignore");
        if global_path.is_file() {
            builder.add_ignore(global_path);
        }
    }

    // 2. Per-directory .agentignore (searched at every depth by the ignore crate)
    builder.add_custom_ignore_filename(".agentignore");
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
}
