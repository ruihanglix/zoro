// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Routing strategy engine: selects the next upstream provider for a request.

use crate::config::{RoutingStrategy, UpstreamProvider};
use crate::health::HealthTracker;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Virtual model name used by the Lab proxy for automatic routing.
/// When a request targets this model, the router bypasses exact-model matching
/// and picks any healthy provider.
const LAB_AUTO_MODEL: &str = "__lab_auto__";

/// Selects an upstream provider + model for a given request.
pub struct Router {
    strategy: std::sync::RwLock<RoutingStrategy>,
    round_robin_counter: AtomicUsize,
}

/// Result of a routing decision.
pub struct RouteTarget<'a> {
    pub provider: &'a UpstreamProvider,
    pub model: String,
}

impl Router {
    pub fn new(strategy: RoutingStrategy) -> Self {
        Self {
            strategy: std::sync::RwLock::new(strategy),
            round_robin_counter: AtomicUsize::new(0),
        }
    }

    pub fn set_strategy(&self, strategy: RoutingStrategy) {
        *self.strategy.write().unwrap() = strategy;
    }

    pub fn strategy(&self) -> RoutingStrategy {
        *self.strategy.read().unwrap()
    }

    /// Select the best provider for the requested model.
    ///
    /// Returns `None` if no healthy provider is available.
    pub fn select<'a>(
        &self,
        requested_model: &str,
        providers: &'a [UpstreamProvider],
        health: &HealthTracker,
    ) -> Option<RouteTarget<'a>> {
        let strategy = self.strategy();
        let is_auto_model = requested_model == LAB_AUTO_MODEL;

        // Build list of all healthy providers (for fallback and auto-routing)
        let all_healthy: Vec<&UpstreamProvider> = providers
            .iter()
            .filter(|p| health.is_healthy(&p.id) && !p.models.is_empty())
            .collect();

        // For the virtual __lab_auto__ model, skip exact-model matching entirely
        // and always pick from all healthy providers.
        if is_auto_model {
            return self.select_auto_model(&all_healthy);
        }

        // Build list of healthy providers (with the requested model)
        let healthy_with_model: Vec<&UpstreamProvider> = providers
            .iter()
            .filter(|p| health.is_healthy(&p.id) && p.models.contains(&requested_model.to_string()))
            .collect();

        match strategy {
            RoutingStrategy::Manual => {
                // Only use providers that have the exact requested model; no fallback
                let provider = healthy_with_model.first()?;
                Some(RouteTarget {
                    provider,
                    model: requested_model.to_string(),
                })
            }

            RoutingStrategy::Auto => {
                // 1) Try a provider that has the exact requested model
                if let Some(provider) = healthy_with_model.first() {
                    return Some(RouteTarget {
                        provider,
                        model: requested_model.to_string(),
                    });
                }
                // 2) Fall back to any healthy provider, picking its first model
                let provider = all_healthy.first()?;
                let model = provider.models.first()?.clone();
                Some(RouteTarget { provider, model })
            }

            RoutingStrategy::RoundRobin => {
                // Cycle through all healthy providers that have any model
                if all_healthy.is_empty() {
                    return None;
                }
                let idx =
                    self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % all_healthy.len();
                let provider = all_healthy[idx];
                // If the provider has the requested model, use it; otherwise pick first
                let model = if provider.models.contains(&requested_model.to_string()) {
                    requested_model.to_string()
                } else {
                    provider.models.first()?.clone()
                };
                Some(RouteTarget { provider, model })
            }
        }
    }

    /// Handle the virtual `__lab_auto__` model: always pick from any healthy provider
    /// using the current strategy, ignoring exact model matching.
    fn select_auto_model<'a>(
        &self,
        all_healthy: &[&'a UpstreamProvider],
    ) -> Option<RouteTarget<'a>> {
        if all_healthy.is_empty() {
            return None;
        }
        let strategy = self.strategy();
        match strategy {
            RoutingStrategy::RoundRobin => {
                let idx =
                    self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % all_healthy.len();
                let provider = all_healthy[idx];
                let model = provider.models.first()?.clone();
                Some(RouteTarget { provider, model })
            }
            // Auto and Manual both pick the first healthy provider's first model
            RoutingStrategy::Auto | RoutingStrategy::Manual => {
                let provider = all_healthy.first()?;
                let model = provider.models.first()?.clone();
                Some(RouteTarget { provider, model })
            }
        }
    }

    /// Select a provider to retry on, excluding certain provider IDs
    /// and optionally certain model names.
    pub fn select_retry<'a>(
        &self,
        requested_model: &str,
        providers: &'a [UpstreamProvider],
        health: &HealthTracker,
        exclude_ids: &[&str],
        exclude_models: &[&str],
    ) -> Option<RouteTarget<'a>> {
        let strategy = self.strategy();
        let is_auto_model = requested_model == LAB_AUTO_MODEL;

        let eligible: Vec<&UpstreamProvider> = providers
            .iter()
            .filter(|p| {
                health.is_healthy(&p.id)
                    && !exclude_ids.contains(&p.id.as_str())
                    && !p.models.is_empty()
            })
            .collect();

        // For __lab_auto__, try any eligible provider with any model not yet tried
        if is_auto_model {
            // First, try to find a model we haven't tried yet across all eligible providers
            for provider in &eligible {
                for model in &provider.models {
                    if !exclude_models.contains(&model.as_str()) {
                        return Some(RouteTarget {
                            provider,
                            model: model.clone(),
                        });
                    }
                }
            }
            // All models tried; fall back to any eligible provider's first model
            // (allows re-trying after backoff)
            let provider = eligible.first()?;
            let model = provider.models.first()?.clone();
            return Some(RouteTarget { provider, model });
        }

        if eligible.is_empty() {
            return None;
        }

        match strategy {
            RoutingStrategy::Manual => {
                // In manual mode, no fallback — return None
                None
            }
            RoutingStrategy::Auto | RoutingStrategy::RoundRobin => {
                // Prefer a provider with the exact model
                if let Some(p) = eligible
                    .iter()
                    .find(|p| p.models.contains(&requested_model.to_string()))
                {
                    return Some(RouteTarget {
                        provider: p,
                        model: requested_model.to_string(),
                    });
                }
                // Otherwise, pick first available with any model
                let provider = eligible.first()?;
                let model = provider.models.first()?.clone();
                Some(RouteTarget { provider, model })
            }
        }
    }
}
