// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Local OpenAI-compatible LLM reverse proxy with multi-provider routing,
//! automatic retry/fallback on 429/5xx, and provider health tracking.

pub mod config;
pub mod error;
pub mod health;
pub mod router;
pub mod server;

pub use config::{ApiFormat, ProxyConfig, RoutingStrategy, UpstreamProvider};
pub use error::ProxyError;
pub use health::ProviderHealthStatus;
pub use server::ProxyServer;
