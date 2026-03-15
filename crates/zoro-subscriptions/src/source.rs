// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::SubscriptionError;
use async_trait::async_trait;
use zoro_core::models::SubscriptionItem;

/// Trait that subscription source plugins implement
#[async_trait]
pub trait SubscriptionSource: Send + Sync {
    /// Unique identifier for this source type (e.g., "huggingface-daily")
    fn source_type(&self) -> &str;

    /// Human-readable display name
    fn display_name(&self) -> &str;

    /// Fetch new items, optionally since a given timestamp
    async fn fetch(
        &self,
        config: &serde_json::Value,
        since: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<SubscriptionItem>, SubscriptionError>;

    /// Default configuration for this source
    fn default_config(&self) -> serde_json::Value;

    /// Description of what this source provides
    fn description(&self) -> &str;
}
