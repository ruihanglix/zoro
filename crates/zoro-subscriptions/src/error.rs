// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SubscriptionError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Source not found: {0}")]
    SourceNotFound(String),

    #[error("Subscription error: {0}")]
    Other(String),
}
