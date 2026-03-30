// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcpProxyError {
    #[error("Server bind error: {0}")]
    Bind(String),

    #[error("ACP error: {0}")]
    Acp(#[from] zoro_acp::AcpError),

    #[error("All workers busy or unavailable")]
    NoWorkerAvailable,

    #[error("Worker pool not initialized")]
    PoolNotReady,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
