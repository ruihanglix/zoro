// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Free LLM service management for Zoro.
//!
//! This crate manages "free AI model" providers (OpenRouter, Groq, Gemini, etc.),
//! handles API key storage, model list fetching/caching, and converts the lab
//! configuration into `zoro_llm_proxy::ProxyConfig` for the proxy server.

pub mod models;
pub mod providers;
pub mod service;

pub use providers::{free_providers, FreeProvider, ProviderTier, FREE_PROVIDERS};
pub use service::{LabConfig, LabService};
