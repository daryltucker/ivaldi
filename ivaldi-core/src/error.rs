use thiserror::Error;
use std::path::PathBuf;

/// Core error types for ivaldi-core.
/// 
/// These focus on machine-readable categories that help agents
/// decide on recovery strategies.
#[derive(Error, Debug)]
pub enum IvaldiError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Systemd error: {0}")]
    Systemd(systemd::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Refactoring error: {0}")]
    Refactoring(String),

    #[error("Journal error: {0}")]
    Journal(String),

    #[error("Undo error: {0}")]
    Undo(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Regex error: {0}")]
    Regex(String),

    #[error("Binary file detected: {0}")]
    BinaryDetected(PathBuf),

    #[error("File too large: {0}")]
    FileTooLarge(PathBuf),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IvaldiError {
    pub fn code(&self) -> &str {
        match self {
            IvaldiError::FileNotFound(_) => "file_not_found",
            IvaldiError::PermissionDenied(_) => "permission_denied",
            IvaldiError::BinaryDetected(_) => "binary_detected",
            IvaldiError::FileTooLarge(_) => "file_too_large",
            IvaldiError::Io(_) => "io_error",
            IvaldiError::Serialization(_) => "serialization_error",
            IvaldiError::Git(_) => "git_error",
            IvaldiError::Systemd(_) => "syslog_error",
            IvaldiError::InvalidArgument(_) => "invalid_arg",
            IvaldiError::Session(_) => "session_error",
            IvaldiError::Journal(_) => "journal_error",
            IvaldiError::Undo(_) => "undo_error",
            IvaldiError::Query(_) => "query_error",
            IvaldiError::Regex(_) => "regex_error",
            IvaldiError::Refactoring(_) => "refactoring_error",
            IvaldiError::Internal(_) => "internal_error",
        }
    }
}
