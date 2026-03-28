// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

//! Worker pool: manages multiple ACP Agent sessions for concurrent request processing.
//! Each worker is an independent ACP Agent process with its own session.
//! Text chunks from each worker are collected via a global buffer keyed by worker name.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde::Serialize;
use tokio::sync::{mpsc, oneshot, Mutex};
use zoro_acp::{AcpManager, AgentUpdate};

use crate::error::AcpProxyError;

// ── Global text buffer for collecting TextChunks per worker ──────────────────

/// Global buffer: worker_name → accumulated text from TextChunk events.
/// Before each request the buffer is cleared; after send_prompt completes
/// the collected text is read and returned.
static TEXT_BUFFERS: std::sync::LazyLock<StdMutex<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Build a callback that appends TextChunk content to the global buffer.
pub fn make_worker_callback(worker_name: String) -> impl Fn(AgentUpdate) + Send + Sync + 'static {
    move |update: AgentUpdate| {
        if let AgentUpdate::TextChunk { text, .. } = update {
            if let Ok(mut buf) = TEXT_BUFFERS.lock() {
                if let Some(buffer) = buf.get_mut(&worker_name) {
                    buffer.push_str(&text);
                }
            }
        }
    }
}

// ── Worker status ────────────────────────────────────────────────────────────

/// Observable status of an individual worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    /// Worker is starting up / connecting to ACP agent.
    Warming,
    /// Worker is idle and ready to process requests.
    Idle,
    /// Worker is currently processing a request.
    Busy,
    /// Worker encountered an error and is not available.
    Error,
}

/// Snapshot of a single worker's state.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerInfo {
    pub index: usize,
    pub status: WorkerStatus,
}

// ── Internal request type ────────────────────────────────────────────────────

struct ChatRequest {
    prompt: String,
    reply: oneshot::Sender<Result<String, String>>,
}

// ── Worker pool ──────────────────────────────────────────────────────────────

/// A pool of ACP Agent workers that process OpenAI-compatible chat requests.
pub struct WorkerPool {
    request_tx: mpsc::UnboundedSender<ChatRequest>,
    worker_statuses: Arc<Vec<StdMutex<WorkerStatus>>>,
    queue_size: Arc<AtomicUsize>,
    worker_count: usize,
    /// Worker names for cleanup on shutdown.
    worker_names: Vec<String>,
}

impl WorkerPool {
    /// Start a new worker pool. Each worker starts an independent ACP Agent
    /// session in the background. Workers begin in `Warming` state and
    /// transition to `Idle` once the ACP session is established.
    /// `config_overrides` is a list of (config_id, value) pairs that will be
    /// applied to every worker session after it starts (e.g. mode + model).
    pub async fn start(
        acp_manager: Arc<Mutex<AcpManager>>,
        agent_config: zoro_acp::AgentConfig,
        config_overrides: Vec<(String, String)>,
        worker_count: usize,
    ) -> Result<Self, AcpProxyError> {
        let worker_count = worker_count.max(1);

        let (request_tx, request_rx) = mpsc::unbounded_channel::<ChatRequest>();
        let request_rx = Arc::new(Mutex::new(request_rx));

        let worker_statuses: Arc<Vec<StdMutex<WorkerStatus>>> = Arc::new(
            (0..worker_count)
                .map(|_| StdMutex::new(WorkerStatus::Warming))
                .collect(),
        );

        let queue_size = Arc::new(AtomicUsize::new(0));
        let mut worker_names = Vec::with_capacity(worker_count);

        for i in 0..worker_count {
            let name = format!("__acp_proxy_worker_{}", i);
            worker_names.push(name.clone());

            // Pre-register the text buffer for this worker
            {
                let mut buf = TEXT_BUFFERS.lock().unwrap();
                buf.insert(name.clone(), String::new());
            }

            let rx = request_rx.clone();
            let statuses = worker_statuses.clone();
            let manager = acp_manager.clone();
            let config = agent_config.clone();
            let qs = queue_size.clone();
            let overrides = config_overrides.clone();

            tokio::spawn(async move {
                run_worker(i, name, rx, statuses, manager, config, overrides, qs).await;
            });
        }

        Ok(Self {
            request_tx,
            worker_statuses,
            queue_size,
            worker_count,
            worker_names,
        })
    }

    /// Send a chat completion request to the pool. Returns a receiver for
    /// the response. Requests are queued FIFO and processed by the first
    /// available worker.
    pub fn send_request(&self, prompt: String) -> oneshot::Receiver<Result<String, String>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.queue_size.fetch_add(1, Ordering::Relaxed);
        let _ = self.request_tx.send(ChatRequest {
            prompt,
            reply: reply_tx,
        });
        reply_rx
    }

    /// Get current status of all workers.
    pub fn worker_infos(&self) -> Vec<WorkerInfo> {
        self.worker_statuses
            .iter()
            .enumerate()
            .map(|(i, s)| WorkerInfo {
                index: i,
                status: *s.lock().unwrap_or_else(|e| e.into_inner()),
            })
            .collect()
    }

    /// Number of requests waiting in the queue.
    pub fn queue_size(&self) -> usize {
        self.queue_size.load(Ordering::Relaxed)
    }

    /// Number of workers.
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Worker names for external use (e.g. stopping sessions).
    pub fn worker_names(&self) -> &[String] {
        &self.worker_names
    }
}

// ── Worker loop ──────────────────────────────────────────────────────────────

async fn run_worker(
    index: usize,
    worker_name: String,
    request_rx: Arc<Mutex<mpsc::UnboundedReceiver<ChatRequest>>>,
    statuses: Arc<Vec<StdMutex<WorkerStatus>>>,
    acp_manager: Arc<Mutex<AcpManager>>,
    agent_config: zoro_acp::AgentConfig,
    config_overrides: Vec<(String, String)>,
    queue_size: Arc<AtomicUsize>,
) {
    tracing::info!(
        worker = index,
        name = %worker_name,
        agent = %agent_config.name,
        "ACP Proxy worker starting"
    );

    // Phase 1: Start the ACP session with a text-collecting callback
    let callback_name = worker_name.clone();
    let session_result = {
        let mgr = acp_manager.lock().await;
        mgr.start_session(
            // Use a modified config with the worker name as the agent name
            // so each worker gets its own independent session.
            &zoro_acp::AgentConfig {
                name: worker_name.clone(),
                ..agent_config.clone()
            },
            None,
            make_worker_callback(callback_name),
        )
        .await
    };

    match session_result {
        Ok(session_id) => {
            tracing::info!(
                worker = index,
                session_id = %session_id,
                "ACP Proxy worker session established"
            );

            // Apply config overrides (mode, model, etc.) with retry.
            // ACP agents (especially OpenCode) may need a moment after session
            // creation before they accept config changes. We retry a few times
            // with increasing delays to handle this race.
            if !config_overrides.is_empty() {
                // Give the agent a moment to finish its internal init
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                const MAX_RETRIES: usize = 3;
                const RETRY_DELAY_MS: u64 = 1000;

                for (ref cid, ref val) in &config_overrides {
                    if cid.is_empty() || val.is_empty() {
                        continue;
                    }
                    let mut success = false;
                    for attempt in 0..MAX_RETRIES {
                        let mgr = acp_manager.lock().await;
                        match mgr.set_config_option(&worker_name, cid, val).await {
                            Ok(_) => {
                                tracing::info!(
                                    worker = index,
                                    config_id = %cid,
                                    value = %val,
                                    "Config option applied on ACP Proxy worker"
                                );
                                success = true;
                                break;
                            }
                            Err(e) => {
                                if attempt + 1 < MAX_RETRIES {
                                    tracing::info!(
                                        worker = index,
                                        config_id = %cid,
                                        attempt = attempt + 1,
                                        error = %e,
                                        "Retrying set_config_option on ACP Proxy worker"
                                    );
                                    drop(mgr);
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        RETRY_DELAY_MS * (attempt as u64 + 1),
                                    ))
                                    .await;
                                } else {
                                    tracing::error!(
                                        worker = index,
                                        config_id = %cid,
                                        value = %val,
                                        error = %e,
                                        "Failed to set config option on ACP Proxy worker after {} retries",
                                        MAX_RETRIES
                                    );
                                }
                            }
                        }
                    }
                    if !success {
                        tracing::warn!(
                            worker = index,
                            config_id = %cid,
                            "Config option could not be applied — agent may use default settings"
                        );
                    }
                }
            }

            set_status(&statuses, index, WorkerStatus::Idle);
        }
        Err(e) => {
            tracing::error!(
                worker = index,
                error = %e,
                "ACP Proxy worker failed to start"
            );
            set_status(&statuses, index, WorkerStatus::Error);
            return;
        }
    }

    // Phase 2: Process requests from the shared queue
    loop {
        let request = {
            let mut rx = request_rx.lock().await;
            rx.recv().await
        };

        let request = match request {
            Some(r) => r,
            None => {
                tracing::info!(worker = index, "ACP Proxy worker shutting down");
                break;
            }
        };

        queue_size.fetch_sub(1, Ordering::Relaxed);
        set_status(&statuses, index, WorkerStatus::Busy);

        // Clear the text buffer before sending the prompt
        {
            let mut buf = TEXT_BUFFERS.lock().unwrap();
            if let Some(buffer) = buf.get_mut(&worker_name) {
                buffer.clear();
            }
        }

        // Send the prompt and wait for completion
        let result = {
            let mgr = acp_manager.lock().await;
            mgr.send_prompt(&worker_name, &request.prompt, vec![]).await
        };

        let response = match result {
            Ok(_stop_reason) => {
                let buf = TEXT_BUFFERS.lock().unwrap();
                let text = buf.get(&worker_name).cloned().unwrap_or_default();
                Ok(text.trim().to_string())
            }
            Err(e) => {
                tracing::warn!(
                    worker = index,
                    error = %e,
                    "ACP Proxy worker prompt failed"
                );
                Err(format!("ACP agent error: {}", e))
            }
        };

        let _ = request.reply.send(response);
        set_status(&statuses, index, WorkerStatus::Idle);
    }

    // Cleanup
    {
        let mut buf = TEXT_BUFFERS.lock().unwrap();
        buf.remove(&worker_name);
    }
    let mgr = acp_manager.lock().await;
    let _ = mgr.stop_session(&worker_name).await;
}

fn set_status(statuses: &[StdMutex<WorkerStatus>], index: usize, status: WorkerStatus) {
    if let Some(s) = statuses.get(index) {
        *s.lock().unwrap_or_else(|e| e.into_inner()) = status;
    }
}
