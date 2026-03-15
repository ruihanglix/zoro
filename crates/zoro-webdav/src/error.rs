// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WebDavError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("WebDAV server returned status {status}: {message}")]
    ServerError { status: u16, message: String },

    #[error("Authentication failed: invalid credentials")]
    AuthenticationFailed,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Conflict: resource already exists at {0}")]
    Conflict(String),

    #[error("XML parse error: {0}")]
    XmlParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Rate limited: too many requests, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("Connection test failed: {0}")]
    ConnectionTestFailed(String),

    #[error("Sync conflict: {0}")]
    SyncConflict(String),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Lock contention: sync-state.json is being modified by another device")]
    LockContention,

    #[error("{0}")]
    Other(String),
}

impl WebDavError {
    /// Check if this error is retryable (transient network issues)
    pub fn is_retryable(&self) -> bool {
        match self {
            WebDavError::HttpError(_) | WebDavError::RateLimited { .. } => true,
            WebDavError::ServerError { status, .. } => *status >= 500,
            _ => false,
        }
    }
}
