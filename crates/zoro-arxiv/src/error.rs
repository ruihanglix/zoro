// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArxivError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("No arXiv ID found for this paper")]
    NoArxivId,

    #[error("Fetch failed: {0}")]
    Fetch(String),

    #[error("HTML file not found: {0}")]
    HtmlNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTML parse error: {0}")]
    ParseError(String),

    #[error("CSS download failed: HTTP {0}")]
    CssDownloadFailed(u16),

    #[error("Translation error: {0}")]
    Translation(String),

    #[error("AI not configured: {0}")]
    AiNotConfigured(String),
}
