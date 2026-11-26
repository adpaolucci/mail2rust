use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OutlookError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Corrupt binary structure: {0}")]
    CorruptStructure(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("Access denied ({scope}): {detail}")]
    AccessDenied { scope: String, detail: String },

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type OutlookResult<T> = Result<T, OutlookError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanWarning {
    pub scope: String,
    pub message: String,
}

impl ScanWarning {
    pub fn new(scope: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            message: message.into(),
        }
    }
}
