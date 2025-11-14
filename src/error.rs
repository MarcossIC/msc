use std::io;
use thiserror::Error;

/// Custom error type for MSC application
#[derive(Error, Debug)]
pub enum MscError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("Elevation required: {0}")]
    ElevationRequired(String),

    #[error("{0}")]
    Other(String),
}

/// Result type alias for MSC application
pub type Result<T> = std::result::Result<T, MscError>;

impl MscError {
    /// Create a config error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        MscError::Config(msg.into())
    }

    /// Create a workspace error
    pub fn workspace<S: Into<String>>(msg: S) -> Self {
        MscError::Workspace(msg.into())
    }

    /// Create a permission denied error
    pub fn permission_denied<S: Into<String>>(msg: S) -> Self {
        MscError::PermissionDenied(msg.into())
    }

    /// Create an invalid path error
    pub fn invalid_path<S: Into<String>>(msg: S) -> Self {
        MscError::InvalidPath(msg.into())
    }

    /// Create an elevation required error
    pub fn elevation_required<S: Into<String>>(msg: S) -> Self {
        MscError::ElevationRequired(msg.into())
    }

    /// Create a generic error
    pub fn other<S: Into<String>>(msg: S) -> Self {
        MscError::Other(msg.into())
    }
}
