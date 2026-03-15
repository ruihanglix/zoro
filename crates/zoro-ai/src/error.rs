// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("No response content from model")]
    EmptyResponse,

    #[error("AI not configured: {0}")]
    NotConfigured(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Request cancelled")]
    Cancelled,
}
