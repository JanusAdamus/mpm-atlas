//! One error type for the whole workspace; `code()` is the SPEC §15 error code.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("sqlite value: {0}")]
    SqlValue(#[from] rusqlite::types::FromSqlError),
    #[error("registry content not usable: {0}")]
    Registry(String),
    #[error("snapshot format: {0}")]
    Format(String),
    #[error("snapshot integrity: {0}")]
    Integrity(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }

    /// Error code of the HTTP/CLI JSON envelope `{"error": {"code", "message"}}`.
    pub fn code(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "not_found",
            Error::BadRequest(_) => "bad_request",
            Error::NotImplemented(_) => "not_implemented",
            _ => "internal",
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"error": {"code": self.code(), "message": self.to_string()}})
    }
}

pub type Result<T> = std::result::Result<T, Error>;
