// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;

/// Rate limiter for WebDAV requests (compatible with services like Jianguoyun
/// that enforce API rate limits).
///
/// Uses a simple token bucket algorithm with configurable burst and refill rate.
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
}

struct RateLimiterState {
    /// Minimum interval between requests
    min_interval: Duration,
    /// Last request timestamp
    last_request: Option<Instant>,
    /// Maximum burst size
    max_burst: u32,
    /// Current available tokens
    available_tokens: u32,
    /// Last token refill time
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// - `requests_per_second`: maximum sustained request rate
    /// - `max_burst`: maximum burst size
    pub fn new(requests_per_second: f64, max_burst: u32) -> Self {
        let min_interval = if requests_per_second > 0.0 {
            Duration::from_secs_f64(1.0 / requests_per_second)
        } else {
            Duration::ZERO
        };

        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                min_interval,
                last_request: None,
                max_burst,
                available_tokens: max_burst,
                last_refill: Instant::now(),
            })),
        }
    }

    /// Create a rate limiter tuned for Jianguoyun (坚果云) which allows
    /// ~30 requests per minute for free accounts.
    pub fn jianguoyun() -> Self {
        Self::new(0.5, 3)
    }

    /// Create a rate limiter with no limits (for self-hosted servers).
    pub fn unlimited() -> Self {
        Self::new(100.0, 50)
    }

    /// Wait until a request is allowed, then consume one token.
    pub async fn acquire(&self) {
        loop {
            let wait_duration = {
                let mut state = self.state.lock().await;
                // Refill tokens based on elapsed time
                let now = Instant::now();
                let elapsed = now.duration_since(state.last_refill);
                let new_tokens = (elapsed.as_secs_f64() / state.min_interval.as_secs_f64()) as u32;
                if new_tokens > 0 {
                    state.available_tokens =
                        (state.available_tokens + new_tokens).min(state.max_burst);
                    state.last_refill = now;
                }

                if state.available_tokens > 0 {
                    state.available_tokens -= 1;
                    state.last_request = Some(now);
                    return;
                }

                // Calculate how long to wait for next token
                state.min_interval
            };

            debug!(
                wait_ms = wait_duration.as_millis(),
                "Rate limiter: waiting before next request"
            );
            tokio::time::sleep(wait_duration).await;
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}
