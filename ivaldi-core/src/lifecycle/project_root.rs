//! # Project Root Discovery
//!
//! ## PURPOSE
//! Determines the "Project Root" for a given path.
//!
//! ## PHILOSOPHY
//! - **Standard Discovery**: Walk up until we find a project marker (`.git`, `.ivaldi`).
//! - **No Magic**: If no marker found, the starting directory (CWD) is the root.
//! - **Hermetic**: Does not depend on external env vars (unless explicitly added later).

use std::path::{Path, PathBuf};

/// Options for root discovery (extensible for future)
#[derive(Debug, Default, Clone)]
pub struct DiscoveryOptions {
    // Add flags here if needed, e.g., strict mode
}

/// Find the project root starting from `start_path`.
///
/// Algorithm:
/// 1. Walk up parent directories.
/// 2. Check for existence of `.ivaldi` (highest priority).
/// 3. Check for existence of `.git`.
/// 4. If root is reached without match, return `start_path`.
pub fn find_project_root(start_path: &Path) -> PathBuf {
    let mut current = start_path;

    // First pass: Make absolute if possible? 
    // Usually start_path comes from std::env::current_dir() which is absolute.
    // If relative, discovery might be weird, but let's assume caller handles absoluteness or we work with relative.
    
    // Walk up
    loop {
        // Check for .ivaldi (Explicit override or existing session)
        if current.join(".ivaldi").exists() {
            return current.to_path_buf();
        }

        // Check for .git (Standard project root)
        if current.join(".git").exists() {
            return current.to_path_buf();
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break, // Reached filesystem root
        }
    }

    // Fallback: Start path (CWD equivalent)
    start_path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_finds_git_root() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("my_project");
        let sub_dir = project_root.join("src/nested");
        
        fs::create_dir_all(&sub_dir).unwrap();
        fs::create_dir(project_root.join(".git")).unwrap();

        let found = find_project_root(&sub_dir);
        assert_eq!(found, project_root);
    }

    #[test]
    fn test_finds_ivaldi_root() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("secret_project");
        let sub_dir = project_root.join("docs");
        
        fs::create_dir_all(&sub_dir).unwrap();
        fs::create_dir(project_root.join(".ivaldi")).unwrap();

        let found = find_project_root(&sub_dir);
        assert_eq!(found, project_root);
    }

    #[test]
    #[ignore]
    fn test_fallback_to_start() {
        let dir = tempdir().unwrap();
        let sub_dir = dir.path().join("random_folder");
        fs::create_dir_all(&sub_dir).unwrap();

        // No markers created

        let found = find_project_root(&sub_dir);
        assert_eq!(found, sub_dir);
    }
}
