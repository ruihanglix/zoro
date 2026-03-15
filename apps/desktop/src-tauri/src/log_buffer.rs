// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const MAX_LOG_ENTRIES: usize = 2000;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

pub type LogBuffer = Arc<Mutex<VecDeque<LogEntry>>>;

pub fn new_log_buffer() -> LogBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)))
}

/// A tracing Layer that captures log entries into a ring buffer
/// and optionally emits them to the Tauri frontend via events.
#[derive(Clone)]
pub struct BufferLayer {
    buffer: LogBuffer,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
}

impl BufferLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self {
            buffer,
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the Tauri AppHandle so we can emit events to the frontend.
    /// Called after `app.setup()` completes.
    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        if let Ok(mut h) = self.app_handle.lock() {
            *h = Some(handle);
        }
    }
}

/// Visitor that extracts the message field from tracing events.
struct MessageVisitor {
    message: String,
}

impl MessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            // Accumulate non-message fields as fallback
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            self.message
                .push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

impl<S: Subscriber> Layer<S> for BufferLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);

        let entry = LogEntry {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message,
        };

        // Push into ring buffer
        if let Ok(mut buf) = self.buffer.lock() {
            if buf.len() >= MAX_LOG_ENTRIES {
                buf.pop_front();
            }
            buf.push_back(entry.clone());
        }

        // Emit to frontend if app handle is available
        if let Ok(handle) = self.app_handle.lock() {
            if let Some(ref app) = *handle {
                use tauri::Emitter;
                let _ = app.emit("log-entry", &entry);
            }
        }
    }
}
