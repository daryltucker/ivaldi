use std::path::{Path, PathBuf};
use std::fs::{self, File};
use anyhow::Result;
use sha2::{Sha256, Digest};

/// Helper: Create backup in .ivaldi/backups
pub fn create_backup(root: &Path, path: &Path) -> Result<(PathBuf, String)> {
    // 1. Calculate Hash
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash = format!("{:x}", hasher.finalize());
    
    // 2. Determine Backup Path
    // Convention: .ivaldi/backups/<hash>_<name>
    // Use project root to find .ivaldi
    let ivaldi_dir = root.join(".ivaldi");
    let backup_dir = ivaldi_dir.join("backups");
    fs::create_dir_all(&backup_dir)?;
    
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let backup_path = backup_dir.join(format!("{}_{}", hash, filename));
    
    // 3. Copy
    // If exists, skipping is fine (CAS).
    if !backup_path.exists() {
        fs::copy(path, &backup_path)?;
    }
    
    Ok((backup_path, hash))
}

pub fn sha256_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
