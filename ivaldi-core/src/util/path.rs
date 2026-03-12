use std::path::{Path, PathBuf};

/// Expands a leading tilde (`~`) in a path into the user's home directory.
/// If the path doesn't start with `~` or the home directory cannot be found,
/// returns the original path.
pub fn expand_tilde(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Ok(home) = std::env::var("HOME") {
            let mut expanded = PathBuf::from(home);
            expanded.push(stripped);
            return expanded;
        }
    }
    
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        
        // Simple case: ~/Documents
        let path = PathBuf::from("~/Documents");
        let expanded = expand_tilde(&path);
        let expected = PathBuf::from(&home).join("Documents");
        assert_eq!(expanded, expected);
        
        // Root case: ~
        let path = PathBuf::from("~");
        let expanded = expand_tilde(&path);
        let expected = PathBuf::from(&home);
        assert_eq!(expanded, expected);
        
        // Non-tilde case: /tmp/test
        let path = PathBuf::from("/tmp/test");
        let expanded = expand_tilde(&path);
        assert_eq!(expanded, path);
    }
}
