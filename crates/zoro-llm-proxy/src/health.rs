// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// After this many consecutive failures, put the provider on cooldown.
const COOLDOWN_FAIL_THRESHOLD: usize = 3;

/// Cooldown duration (5 minutes).
const COOLDOWN_DURATION: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Health record for a single upstream provider. Thread-safe.
pub struct ProviderHealth {
    pub last_success: std::sync::Mutex<Option<Instant>>,
    pub last_failure: std::sync::Mutex<Option<Instant>>,
    pub consecutive_fails: AtomicUsize,
    pub cooldown_until: std::sync::Mutex<Option<Instant>>,
    pub total_requests: AtomicUsize,
    pub total_failures: AtomicUsize,
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderHealth {
    pub fn new() -> Self {
        Self {
            last_success: std::sync::Mutex::new(None),
            last_failure: std::sync::Mutex::new(None),
            consecutive_fails: AtomicUsize::new(0),
            cooldown_until: std::sync::Mutex::new(None),
            total_requests: AtomicUsize::new(0),
            total_failures: AtomicUsize::new(0),
        }
    }

    /// Record a successful request.
    pub fn report_success(&self) {
        *self.last_success.lock().unwrap() = Some(Instant::now());
        self.consecutive_fails.store(0, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed request. May trigger cooldown.
    pub fn report_failure(&self) {
        let now = Instant::now();
        *self.last_failure.lock().unwrap() = Some(now);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_failures.fetch_add(1, Ordering::Relaxed);
        let fails = self.consecutive_fails.fetch_add(1, Ordering::Relaxed) + 1;
        if fails >= COOLDOWN_FAIL_THRESHOLD {
            *self.cooldown_until.lock().unwrap() = Some(now + COOLDOWN_DURATION);
        }
    }

    /// Record a rate-limited (429) response. Tracks the event but does NOT
    /// increment consecutive failures and will never trigger cooldown.
    /// Rate limiting is a transient traffic signal, not an indicator that
    /// the provider is unhealthy.
    pub fn report_rate_limited(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        // Do NOT touch consecutive_fails or cooldown_until.
    }

    /// Check whether this provider is currently healthy (not on cooldown).
    pub fn is_healthy(&self) -> bool {
        let guard = self.cooldown_until.lock().unwrap();
        match *guard {
            Some(until) => Instant::now() >= until,
            None => true,
        }
    }
}

/// Snapshot of a provider's health status for external queries (UI, API).
#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealthStatus {
    pub provider_id: String,
    pub healthy: bool,
    pub consecutive_fails: usize,
    pub total_requests: usize,
    pub total_failures: usize,
    pub on_cooldown: bool,
}

/// Manages health records for all upstream providers.
pub struct HealthTracker {
    records: std::sync::RwLock<HashMap<String, ProviderHealth>>,
}

impl HealthTracker {
    pub fn new() -> Self {
        Self {
            records: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Ensure a health record exists for the given provider.
    pub fn ensure_provider(&self, provider_id: &str) {
        let read = self.records.read().unwrap();
        if read.contains_key(provider_id) {
            return;
        }
        drop(read);
        let mut write = self.records.write().unwrap();
        write.entry(provider_id.to_string()).or_default();
    }

    pub fn report_success(&self, provider_id: &str) {
        let read = self.records.read().unwrap();
        if let Some(h) = read.get(provider_id) {
            h.report_success();
        }
    }

    pub fn report_failure(&self, provider_id: &str) {
        let read = self.records.read().unwrap();
        if let Some(h) = read.get(provider_id) {
            h.report_failure();
        }
    }

    pub fn report_rate_limited(&self, provider_id: &str) {
        let read = self.records.read().unwrap();
        if let Some(h) = read.get(provider_id) {
            h.report_rate_limited();
        }
    }

    pub fn is_healthy(&self, provider_id: &str) -> bool {
        let read = self.records.read().unwrap();
        read.get(provider_id)
            .map(|h| h.is_healthy())
            .unwrap_or(true)
    }

    /// Get a snapshot of all provider health statuses.
    pub fn all_statuses(&self) -> Vec<ProviderHealthStatus> {
        let read = self.records.read().unwrap();
        read.iter()
            .map(|(id, h)| {
                let on_cooldown = !h.is_healthy();
                ProviderHealthStatus {
                    provider_id: id.clone(),
                    healthy: !on_cooldown,
                    consecutive_fails: h.consecutive_fails.load(Ordering::Relaxed),
                    total_requests: h.total_requests.load(Ordering::Relaxed),
                    total_failures: h.total_failures.load(Ordering::Relaxed),
                    on_cooldown,
                }
            })
            .collect()
    }

    /// Reset all health records (e.g. when providers are reconfigured).
    pub fn reset(&self) {
        let mut write = self.records.write().unwrap();
        write.clear();
    }
}

impl Default for HealthTracker {
    fn default() -> Self {
        Self::new()
    }
}
