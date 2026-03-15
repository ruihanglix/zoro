// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod config;

// Re-export shared storage modules from zoro-storage crate.
pub use zoro_storage::attachments;
pub use zoro_storage::paper_dir;
pub use zoro_storage::sync;

pub use zoro_storage::init_data_dir;
