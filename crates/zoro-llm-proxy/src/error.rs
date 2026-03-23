// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("No healthy upstream provider available")]
    NoHealthyProvider,

    #[error("Provider '{0}' not found")]
    ProviderNotFound(String),

    #[error("No provider has model '{0}'")]
    ModelNotFound(String),

    #[error("Upstream error (status {status}): {body}")]
    Upstream { status: u16, body: String },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Server bind error: {0}")]
    Bind(String),

    #[error("All retries exhausted after {attempts} attempts")]
    RetriesExhausted { attempts: usize },
}
