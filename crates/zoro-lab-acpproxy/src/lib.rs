// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! ACP Proxy: converts ACP Agent sessions into an OpenAI-compatible local API server.
//!
//! This crate manages a pool of ACP Agent workers and exposes them through
//! a standard `/v1/chat/completions` endpoint, allowing existing translation
//! and chat features to use ACP Agents without code changes.

pub mod config;
pub mod error;
pub mod server;
pub mod worker;

pub use config::AcpProxyConfig;
pub use error::AcpProxyError;
pub use server::AcpProxyServer;
pub use worker::{WorkerInfo, WorkerPool, WorkerStatus};
