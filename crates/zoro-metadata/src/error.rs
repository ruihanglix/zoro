// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(String),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("PDF parse error: {0}")]
    PdfParse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
