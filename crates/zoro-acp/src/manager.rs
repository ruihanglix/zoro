// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use agent_client_protocol::{
    Agent, AuthenticateRequest, CancelNotification, ClientSideConnection, ContentBlock,
    ImageContent, Implementation, InitializeRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    StopReason, TextContent, ToolCallContent,
};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::error::AcpError;

// ── Types emitted to frontend ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum AgentUpdate {
    #[serde(rename = "text_chunk")]
    TextChunk { session_id: String, text: String },
    #[serde(rename = "thought_chunk")]
    ThoughtChunk { session_id: String, text: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        session_id: String,
        tool_call_id: String,
        title: String,
        status: String,
        raw_input: Option<String>,
        raw_output: Option<String>,
    },
    #[serde(rename = "tool_call_update")]
    ToolCallUpdate {
        session_id: String,
        tool_call_id: String,
        status: String,
        content_text: Option<String>,
        raw_input: Option<String>,
        raw_output: Option<String>,
    },
    #[serde(rename = "plan")]
    Plan {
        session_id: String,
        entries: Vec<PlanEntryUpdate>,
    },
    #[serde(rename = "prompt_done")]
    PromptDone {
        session_id: String,
        stop_reason: String,
    },
    #[serde(rename = "config_options")]
    ConfigOptions {
        session_id: String,
        config_options: Vec<ConfigOptionInfo>,
    },
    #[serde(rename = "error")]
    Error { session_id: String, message: String },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanEntryUpdate {
    pub content: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigOptionValue {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigOptionInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub current_value: String,
    pub options: Vec<ConfigOptionValue>,
}

fn convert_config_options(opts: &[SessionConfigOption]) -> Vec<ConfigOptionInfo> {
    opts.iter()
        .filter_map(|opt| {
            let SessionConfigKind::Select(ref sel) = opt.kind else {
                return None;
            };
            let values: Vec<ConfigOptionValue> = match &sel.options {
                SessionConfigSelectOptions::Ungrouped(list) => list
                    .iter()
                    .map(|o| ConfigOptionValue {
                        value: o.value.to_string(),
                        name: o.name.clone(),
                        description: o.description.clone(),
                    })
                    .collect(),
                SessionConfigSelectOptions::Grouped(groups) => groups
                    .iter()
                    .flat_map(|g| &g.options)
                    .map(|o| ConfigOptionValue {
                        value: o.value.to_string(),
                        name: o.name.clone(),
                        description: o.description.clone(),
                    })
                    .collect(),
                _ => return None,
            };
            let category = opt.category.as_ref().map(|c| match c {
                SessionConfigOptionCategory::Mode => "mode".into(),
                SessionConfigOptionCategory::Model => "model".into(),
                SessionConfigOptionCategory::ThoughtLevel => "thought_level".into(),
                SessionConfigOptionCategory::Other(s) => s.clone(),
                _ => "other".into(),
            });
            Some(ConfigOptionInfo {
                id: opt.id.to_string(),
                name: opt.name.clone(),
                description: opt.description.clone(),
                category,
                current_value: sel.current_value.to_string(),
                options: values,
            })
        })
        .collect()
}

// ── Internal command channel ─────────────────────────────────────────────────

enum AgentCommand {
    Prompt {
        text: String,
        images: Vec<ImageAttachment>,
        reply: oneshot::Sender<Result<String, AcpError>>,
    },
    Cancel {
        reply: oneshot::Sender<Result<(), AcpError>>,
    },
    SetConfigOption {
        config_id: String,
        value: String,
        reply: oneshot::Sender<Result<Vec<ConfigOptionInfo>, AcpError>>,
    },
    Stop,
}

struct AgentHandle {
    cmd_tx: mpsc::UnboundedSender<AgentCommand>,
    #[allow(dead_code)]
    session_id: String,
    _thread: std::thread::JoinHandle<()>,
}

// ── Public API ───────────────────────────────────────────────────────────────

pub struct AcpManager {
    connections: Mutex<HashMap<String, AgentHandle>>,
    data_dir: PathBuf,
}

impl AcpManager {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            data_dir,
        }
    }

    pub async fn start_session<F>(
        &self,
        agent_config: &crate::config::AgentConfig,
        working_dir: Option<String>,
        on_update: F,
    ) -> Result<String, AcpError>
    where
        F: Fn(AgentUpdate) + Send + Sync + 'static,
    {
        let on_update = Arc::new(on_update);
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (init_tx, init_rx) = oneshot::channel::<Result<String, AcpError>>();

        let command = agent_config.command.clone();
        let args = agent_config.args.clone();
        let env_vars: Vec<(String, String)> = agent_config
            .env
            .iter()
            .map(|e| (e.name.clone(), e.value.clone()))
            .collect();
        let agent_name = agent_config.name.clone();
        let cwd = working_dir
            .unwrap_or_else(|| self.data_dir.join("library").to_string_lossy().to_string());

        let thread = std::thread::Builder::new()
            .name(format!("acp-{}", agent_name))
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create ACP runtime");

                let local = tokio::task::LocalSet::new();

                local.block_on(&rt, async move {
                    match spawn_agent_connection(
                        &command,
                        &args,
                        &env_vars,
                        &cwd,
                        &agent_name,
                        on_update,
                    )
                    .await
                    {
                        Ok((session_id, connection)) => {
                            let _ = init_tx.send(Ok(session_id.clone()));
                            run_command_loop(cmd_rx, connection, &session_id).await;
                        }
                        Err(e) => {
                            let _ = init_tx.send(Err(e));
                        }
                    }
                });
            })
            .map_err(|e| AcpError::SpawnFailed(e.to_string()))?;

        let session_id = init_rx.await.map_err(|_| AcpError::ProcessExited)??;

        let handle = AgentHandle {
            cmd_tx,
            session_id: session_id.clone(),
            _thread: thread,
        };

        self.connections
            .lock()
            .await
            .insert(agent_config.name.clone(), handle);

        Ok(session_id)
    }

    pub async fn send_prompt(
        &self,
        agent_name: &str,
        text: &str,
        images: Vec<ImageAttachment>,
    ) -> Result<String, AcpError> {
        let connections = self.connections.lock().await;
        let handle = connections
            .get(agent_name)
            .ok_or_else(|| AcpError::SessionNotFound(agent_name.into()))?;

        let (reply_tx, reply_rx) = oneshot::channel();
        handle
            .cmd_tx
            .send(AgentCommand::Prompt {
                text: text.to_string(),
                images,
                reply: reply_tx,
            })
            .map_err(|_| AcpError::ProcessExited)?;

        drop(connections);
        reply_rx.await.map_err(|_| AcpError::ProcessExited)?
    }

    pub async fn set_config_option(
        &self,
        agent_name: &str,
        config_id: &str,
        value: &str,
    ) -> Result<Vec<ConfigOptionInfo>, AcpError> {
        let connections = self.connections.lock().await;
        let handle = connections
            .get(agent_name)
            .ok_or_else(|| AcpError::SessionNotFound(agent_name.into()))?;

        let (reply_tx, reply_rx) = oneshot::channel();
        handle
            .cmd_tx
            .send(AgentCommand::SetConfigOption {
                config_id: config_id.to_string(),
                value: value.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| AcpError::ProcessExited)?;

        drop(connections);
        reply_rx.await.map_err(|_| AcpError::ProcessExited)?
    }

    pub async fn cancel_prompt(&self, agent_name: &str) -> Result<(), AcpError> {
        let connections = self.connections.lock().await;
        let handle = connections
            .get(agent_name)
            .ok_or_else(|| AcpError::SessionNotFound(agent_name.into()))?;

        let (reply_tx, reply_rx) = oneshot::channel();
        handle
            .cmd_tx
            .send(AgentCommand::Cancel { reply: reply_tx })
            .map_err(|_| AcpError::ProcessExited)?;

        drop(connections);
        reply_rx.await.map_err(|_| AcpError::ProcessExited)?
    }

    pub async fn stop_session(&self, agent_name: &str) -> Result<(), AcpError> {
        let mut connections = self.connections.lock().await;
        if let Some(handle) = connections.remove(agent_name) {
            let _ = handle.cmd_tx.send(AgentCommand::Stop);
            tracing::info!("Stopped ACP agent: {}", agent_name);
        }
        Ok(())
    }

    pub async fn stop_all(&self) {
        let mut connections = self.connections.lock().await;
        for (name, handle) in connections.drain() {
            let _ = handle.cmd_tx.send(AgentCommand::Stop);
            tracing::info!("Stopped ACP agent: {}", name);
        }
    }

    pub async fn has_session(&self, agent_name: &str) -> bool {
        self.connections.lock().await.contains_key(agent_name)
    }
}

#[derive(Debug, Clone)]
pub struct ImageAttachment {
    pub base64_data: String,
    pub mime_type: String,
}

// ── Agent thread internals ───────────────────────────────────────────────────

async fn spawn_agent_connection(
    command: &str,
    args: &[String],
    env_vars: &[(String, String)],
    cwd: &str,
    agent_name: &str,
    on_update: Arc<dyn Fn(AgentUpdate) + Send + Sync>,
) -> Result<(String, ClientSideConnection), AcpError> {
    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // Use expanded PATH so agent binaries in user-level dirs are found
    cmd.env("PATH", crate::full_path());
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AcpError::SpawnFailed(format!("{}: {}", command, e)))?;

    let stdin = child.stdin.take().ok_or(AcpError::ProcessExited)?;
    let stdout = child.stdout.take().ok_or(AcpError::ProcessExited)?;

    if let Some(stderr) = child.stderr.take() {
        let name = agent_name.to_string();
        tokio::task::spawn_local(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("[ACP:{}:stderr] {}", name, line);
            }
        });
    }

    // Keep the child process handle alive
    tokio::task::spawn_local(async move {
        let _ = child.wait().await;
    });

    let client = AcpClient {
        on_update: on_update.clone(),
    };

    let (connection, io_task) =
        ClientSideConnection::new(client, stdin.compat_write(), stdout.compat(), |fut| {
            tokio::task::spawn_local(fut);
        });

    tokio::task::spawn_local(async move {
        if let Err(e) = io_task.await {
            tracing::error!("ACP IO task error: {}", e);
        }
    });

    let init_response = connection
        .initialize(
            InitializeRequest::new(ProtocolVersion::V1)
                .client_info(Implementation::new("zoro", "0.1.0")),
        )
        .await
        .map_err(|e| AcpError::Protocol(format!("Initialize failed: {}", e)))?;

    tracing::info!(
        "ACP agent initialized: {:?} (protocol v{:?})",
        init_response.agent_info,
        init_response.protocol_version
    );

    // Authenticate if the agent advertises any auth methods.
    // Some agents (e.g. OpenCode) report auth_methods but don't implement
    // the authenticate RPC, so treat failures as non-fatal.
    if let Some(method) = init_response.auth_methods.first() {
        let method_id = method.id().clone();
        tracing::info!("ACP authenticating with method: {:?}", method_id);
        match connection
            .authenticate(AuthenticateRequest::new(method_id))
            .await
        {
            Ok(_) => tracing::info!("ACP authentication successful"),
            Err(e) => tracing::warn!("ACP authenticate skipped (non-fatal): {}", e),
        }
    }

    let session_response = connection
        .new_session(NewSessionRequest::new(cwd))
        .await
        .map_err(|e| AcpError::Protocol(format!("session/new failed: {}", e)))?;

    let session_id = session_response.session_id.to_string();

    if let Some(ref opts) = session_response.config_options {
        let converted = convert_config_options(opts);
        if !converted.is_empty() {
            (on_update)(AgentUpdate::ConfigOptions {
                session_id: session_id.clone(),
                config_options: converted,
            });
        }
    }

    Ok((session_id, connection))
}

async fn run_command_loop(
    mut cmd_rx: mpsc::UnboundedReceiver<AgentCommand>,
    connection: ClientSideConnection,
    session_id: &str,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            AgentCommand::Prompt {
                text,
                images,
                reply,
            } => {
                let mut prompt_content: Vec<ContentBlock> = Vec::new();

                if !text.is_empty() {
                    prompt_content.push(ContentBlock::Text(TextContent::new(text)));
                }

                for img in images {
                    prompt_content.push(ContentBlock::Image(ImageContent::new(
                        img.base64_data,
                        img.mime_type,
                    )));
                }

                let request = PromptRequest::new(session_id.to_owned(), prompt_content);

                let result = connection.prompt(request).await;
                let _ = reply.send(
                    result
                        .map(|r| stop_reason_str(r.stop_reason))
                        .map_err(|e| AcpError::Protocol(format!("session/prompt failed: {}", e))),
                );
            }
            AgentCommand::Cancel { reply } => {
                let result = connection
                    .cancel(CancelNotification::new(session_id.to_owned()))
                    .await
                    .map_err(|e| AcpError::Protocol(format!("session/cancel failed: {}", e)));
                let _ = reply.send(result);
            }
            AgentCommand::SetConfigOption {
                config_id,
                value,
                reply,
            } => {
                let request =
                    SetSessionConfigOptionRequest::new(session_id.to_owned(), config_id, value);
                let result = connection
                    .set_session_config_option(request)
                    .await
                    .map(|r| convert_config_options(&r.config_options))
                    .map_err(|e| AcpError::Protocol(format!("set_config_option failed: {}", e)));
                let _ = reply.send(result);
            }
            AgentCommand::Stop => {
                break;
            }
        }
    }
}

fn stop_reason_str(r: StopReason) -> String {
    match r {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::MaxTurnRequests => "max_model_requests",
        StopReason::Refusal => "refused",
        StopReason::Cancelled => "cancelled",
        _ => "unknown",
    }
    .to_string()
}

// ── ACP Client trait impl ────────────────────────────────────────────────────

struct AcpClient {
    on_update: Arc<dyn Fn(AgentUpdate) + Send + Sync>,
}

#[async_trait::async_trait(?Send)]
impl agent_client_protocol::Client for AcpClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> agent_client_protocol::Result<RequestPermissionResponse> {
        let title = args.tool_call.fields.title.as_deref().unwrap_or("unknown");
        tracing::info!("ACP permission requested: {}", title);
        let option_id = args.options.first().map(|o| o.option_id.clone());
        let outcome = match option_id {
            Some(id) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(id)),
            None => RequestPermissionOutcome::Cancelled,
        };
        Ok(RequestPermissionResponse::new(outcome))
    }

    async fn session_notification(
        &self,
        args: SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        let session_id = args.session_id.to_string();

        match args.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    (self.on_update)(AgentUpdate::TextChunk {
                        session_id,
                        text: text.text,
                    });
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    (self.on_update)(AgentUpdate::ThoughtChunk {
                        session_id,
                        text: text.text,
                    });
                }
            }
            SessionUpdate::ToolCall(tc) => {
                let status = format!("{:?}", tc.status);
                let raw_input = tc.raw_input.map(|v| serde_json::to_string(&v).unwrap_or_default());
                let raw_output = tc.raw_output.map(|v| serde_json::to_string(&v).unwrap_or_default());
                (self.on_update)(AgentUpdate::ToolCall {
                    session_id,
                    tool_call_id: tc.tool_call_id.to_string(),
                    title: tc.title,
                    status,
                    raw_input,
                    raw_output,
                });
            }
            SessionUpdate::ToolCallUpdate(tcu) => {
                let status = tcu
                    .fields
                    .status
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_default();
                let content_text = tcu.fields.content.and_then(|blocks: Vec<ToolCallContent>| {
                    blocks.into_iter().find_map(|b| match b {
                        ToolCallContent::Content(c) => {
                            if let ContentBlock::Text(t) = c.content {
                                Some(t.text)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                });
                let raw_input = tcu.fields.raw_input.map(|v| serde_json::to_string(&v).unwrap_or_default());
                let raw_output = tcu.fields.raw_output.map(|v| serde_json::to_string(&v).unwrap_or_default());
                (self.on_update)(AgentUpdate::ToolCallUpdate {
                    session_id,
                    tool_call_id: tcu.tool_call_id.to_string(),
                    status,
                    content_text,
                    raw_input,
                    raw_output,
                });
            }
            SessionUpdate::Plan(plan) => {
                let entries = plan
                    .entries
                    .into_iter()
                    .map(|e| PlanEntryUpdate {
                        content: e.content,
                        status: format!("{:?}", e.status),
                    })
                    .collect();
                (self.on_update)(AgentUpdate::Plan {
                    session_id,
                    entries,
                });
            }
            SessionUpdate::ConfigOptionUpdate(update) => {
                let converted = convert_config_options(&update.config_options);
                if !converted.is_empty() {
                    (self.on_update)(AgentUpdate::ConfigOptions {
                        session_id,
                        config_options: converted,
                    });
                }
            }
            _ => {
                tracing::debug!("Unhandled ACP session update for {}", session_id);
            }
        }

        Ok(())
    }
}
