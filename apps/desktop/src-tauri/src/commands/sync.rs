// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info};

use zoro_core::models::SyncConfig;
use zoro_webdav::client::WebDavClient;
use zoro_webdav::file_sync;
use zoro_webdav::rate_limiter::RateLimiter;
use zoro_webdav::sync_engine::{DbHandle, SyncEngine};
use zoro_webdav::types::{SyncProgress, SyncStatus};

use crate::AppState;

/// Shared sync engine state managed by Tauri.
pub struct SyncState {
    pub engine: TokioMutex<Option<SyncEngine>>,
    pub syncing: TokioMutex<bool>,
    pub last_error: TokioMutex<Option<String>>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            engine: TokioMutex::new(None),
            syncing: TokioMutex::new(false),
            last_error: TokioMutex::new(None),
        }
    }
}

/// Create a WebDavClient from SyncConfig.
fn create_client(config: &SyncConfig) -> Result<WebDavClient, String> {
    // Auto-detect rate limiter based on URL
    let rate_limiter = if config.url.contains("jianguoyun") || config.url.contains("nutstore") {
        Some(RateLimiter::jianguoyun())
    } else {
        None
    };

    WebDavClient::new(
        &config.url,
        &config.username,
        &config.password,
        rate_limiter,
    )
    .map_err(|e| format!("Failed to create WebDAV client: {}", e))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Test WebDAV connection with the given credentials.
/// If `password` is empty but a password is already saved in config, use the saved one.
#[tauri::command]
pub async fn test_webdav_connection(
    state: State<'_, AppState>,
    url: String,
    username: String,
    password: String,
) -> Result<String, String> {
    let effective_password = if password.is_empty() {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.sync.password.clone()
    } else {
        password
    };

    let client = WebDavClient::new(&url, &username, &effective_password, None)
        .map_err(|e| format!("Invalid configuration: {}", e))?;

    client
        .test_connection()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    Ok("Connection successful".to_string())
}

/// Save sync configuration and initialize the sync engine.
#[tauri::command]
pub async fn save_sync_config(
    _app: AppHandle,
    state: State<'_, AppState>,
    sync_state: State<'_, SyncState>,
    config: SyncConfig,
) -> Result<(), String> {
    // If the password field is empty, preserve the existing password
    // (the frontend sends empty string when the user hasn't changed it).
    let mut config = config;
    if config.password.is_empty() {
        let existing = state.config.lock().map_err(|e| e.to_string())?;
        config.password = existing.sync.password.clone();
    }

    // Update config in memory
    {
        let mut app_config = state.config.lock().map_err(|e| e.to_string())?;
        app_config.sync = config.clone();

        // Save to config.toml
        let config_path = state.data_dir.join("config.toml");
        let toml_str =
            toml::to_string_pretty(&*app_config).map_err(|e| format!("TOML error: {}", e))?;
        std::fs::write(&config_path, toml_str).map_err(|e| format!("Write error: {}", e))?;
    }

    // Enable sync tracking on the database
    if config.enabled && !config.device_id.is_empty() {
        let mut db = state.db.lock().map_err(|e| e.to_string())?;
        db.enable_sync_tracking(config.device_id.clone());
    }

    // Initialize or tear down the sync engine
    if config.enabled {
        let client = create_client(&config)?;
        let engine = SyncEngine::new(
            client,
            &config.remote_path,
            &config.device_id,
            &config.device_name,
        );
        *sync_state.engine.lock().await = Some(engine);
        info!("Sync engine initialized");
    } else {
        *sync_state.engine.lock().await = None;
        info!("Sync engine disabled");
    }

    Ok(())
}

/// Trigger a manual sync cycle.
/// Automatically detects whether this is the first sync for the device
/// and runs `initial_sync` (full metadata exchange) before the incremental
/// changelog-based `sync`.
#[tauri::command]
pub async fn trigger_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    sync_state: State<'_, SyncState>,
) -> Result<(), String> {
    // Check if already syncing
    {
        let syncing = sync_state.syncing.lock().await;
        if *syncing {
            return Err("Sync is already in progress".to_string());
        }
    }

    let engine_guard = sync_state.engine.lock().await;
    let engine = engine_guard
        .as_ref()
        .ok_or("Sync engine not initialized. Please configure WebDAV sync first.")?;

    // Set syncing flag
    *sync_state.syncing.lock().await = true;
    *sync_state.last_error.lock().await = None;

    let db_handle: DbHandle = state.db.clone();
    let data_dir = state.data_dir.clone();

    // Determine whether this device has ever synced before.
    // If last_sync_time is None, run the initial full sync first.
    let needs_initial_sync = {
        let device_id = {
            let config = state.config.lock().map_err(|e| e.to_string())?;
            config.sync.device_id.clone()
        };
        let db_guard = db_handle.lock().map_err(|e| e.to_string())?;
        let sync_state_row =
            zoro_db::queries::sync::get_or_create_sync_state(&db_guard.conn, &device_id)
                .map_err(|e| e.to_string())?;
        sync_state_row.last_sync_time.is_none()
    };

    let result = if needs_initial_sync {
        info!("First sync detected — running initial full sync");
        let app_handle = app.clone();
        engine
            .initial_sync(
                &db_handle,
                &data_dir,
                Some(|progress: SyncProgress| {
                    let _ = app_handle.emit("sync:progress", &progress);
                }),
            )
            .await
    } else {
        let app_handle = app.clone();
        engine
            .sync(
                &db_handle,
                Some(|progress: SyncProgress| {
                    let _ = app_handle.emit("sync:progress", &progress);
                }),
            )
            .await
    };

    // After metadata sync, run L2 small file sync
    if result.is_ok() {
        let (remote_path, device_id) = {
            let config = state.config.lock().map_err(|e| e.to_string())?;
            (
                config.sync.remote_path.clone(),
                config.sync.device_id.clone(),
            )
        };
        let max_file_size: u64 = {
            let config = state.config.lock().map_err(|e| e.to_string())?;
            if config.sync.max_file_size_mb == 0 {
                0
            } else {
                config.sync.max_file_size_mb as u64 * 1_048_576
            }
        };
        let _ = file_sync::sync_all_small_files(
            engine.client(),
            &remote_path,
            &device_id,
            &data_dir,
            max_file_size,
        )
        .await;
    }

    // Update flags
    *sync_state.syncing.lock().await = false;

    match result {
        Ok(()) => {
            let _ = app.emit("sync:complete", ());
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("{}", e);
            *sync_state.last_error.lock().await = Some(err_msg.clone());
            let _ = app.emit("sync:error", &err_msg);
            Err(err_msg)
        }
    }
}

/// Return the current sync configuration for display in settings.
/// The password is redacted — only a boolean flag `password_set` is exposed.
#[tauri::command]
pub async fn get_sync_config(state: State<'_, AppState>) -> Result<SyncConfigResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let sync = &config.sync;
    Ok(SyncConfigResponse {
        enabled: sync.enabled,
        url: sync.url.clone(),
        username: sync.username.clone(),
        password_set: !sync.password.is_empty(),
        remote_path: sync.remote_path.clone(),
        interval_minutes: sync.interval_minutes,
        device_id: sync.device_id.clone(),
        device_name: sync.device_name.clone(),
        sync_collections: sync.sync_collections,
        sync_tags: sync.sync_tags,
        sync_annotations: sync.sync_annotations,
        sync_reader_state: sync.sync_reader_state,
        sync_notes: sync.sync_notes,
        sync_attachments: sync.sync_attachments,
        max_file_size_mb: sync.max_file_size_mb,
        pdf_download_mode: sync.pdf_download_mode.clone(),
        conflict_strategy: sync.conflict_strategy.clone(),
    })
}

/// Response struct that mirrors SyncConfig but redacts the password.
#[derive(serde::Serialize)]
pub struct SyncConfigResponse {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password_set: bool,
    pub remote_path: String,
    pub interval_minutes: i32,
    pub device_id: String,
    pub device_name: String,
    pub sync_collections: bool,
    pub sync_tags: bool,
    pub sync_annotations: bool,
    pub sync_reader_state: bool,
    pub sync_notes: bool,
    pub sync_attachments: bool,
    pub max_file_size_mb: u32,
    pub pdf_download_mode: String,
    pub conflict_strategy: String,
}

/// Get the current sync status.
#[tauri::command]
pub async fn get_sync_status(
    state: State<'_, AppState>,
    sync_state: State<'_, SyncState>,
) -> Result<SyncStatus, String> {
    let syncing = *sync_state.syncing.lock().await;
    let last_error = sync_state.last_error.lock().await.clone();

    let engine_guard = sync_state.engine.lock().await;

    if let Some(ref engine) = *engine_guard {
        let mut status = engine.get_status(&state.db);
        status.syncing = syncing;
        status.last_error = last_error;
        Ok(status)
    } else {
        Ok(SyncStatus {
            enabled: false,
            syncing: false,
            last_sync_time: None,
            last_error,
            progress: None,
            devices: Vec::new(),
        })
    }
}

/// Download a paper's PDF or HTML file on demand (lazy fetch).
#[tauri::command]
pub async fn download_paper_file(
    app: AppHandle,
    state: State<'_, AppState>,
    sync_state: State<'_, SyncState>,
    paper_id: String,
    file_type: String, // "pdf" or "html"
) -> Result<String, String> {
    use zoro_db::queries::papers;

    let (slug, data_dir, remote_root) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let paper = papers::get_paper(&db.conn, &paper_id)
            .map_err(|e| format!("Paper not found: {}", e))?;
        let config = state.config.lock().map_err(|e| e.to_string())?;
        (
            paper.slug.clone(),
            state.data_dir.clone(),
            config.sync.remote_path.clone(),
        )
    };

    let engine_guard = sync_state.engine.lock().await;
    let engine = engine_guard.as_ref().ok_or("Sync not configured")?;

    let filename = match file_type.as_str() {
        "pdf" => "paper.pdf",
        "html" => "paper.html",
        _ => return Err(format!("Unknown file type: {}", file_type)),
    };

    let remote_path = format!(
        "{}/zoro/library/papers/{}/{}",
        remote_root.trim_end_matches('/'),
        slug,
        filename
    );

    let local_dir = data_dir.join("library/papers").join(&slug);
    std::fs::create_dir_all(&local_dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    let local_path = local_dir.join(filename);

    let app_handle = app.clone();
    let slug_clone = slug.clone();
    let filename_str = filename.to_string();

    let bytes_downloaded = engine
        .client()
        .get_to_file(
            &remote_path,
            &local_path,
            Some(move |downloaded: u64, total: Option<u64>| {
                let _ = app_handle.emit(
                    "sync:download-progress",
                    serde_json::json!({
                        "slug": slug_clone,
                        "filename": filename_str,
                        "downloaded": downloaded,
                        "total": total,
                    }),
                );
            }),
        )
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    // Update the downloaded flag in the database
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        match file_type.as_str() {
            "pdf" => papers::set_pdf_downloaded(&db.conn, &paper_id, true),
            "html" => papers::set_html_downloaded(&db.conn, &paper_id, true),
            _ => Ok(()),
        }
        .map_err(|e| format!("Failed to update download status: {}", e))?;
    }

    let _ = app.emit(
        "sync:download-complete",
        serde_json::json!({
            "slug": slug,
            "filename": filename,
            "size": bytes_downloaded,
        }),
    );

    Ok(local_path.to_string_lossy().to_string())
}

/// Cancel the current sync operation.
#[tauri::command]
pub async fn cancel_sync(sync_state: State<'_, SyncState>) -> Result<(), String> {
    let engine_guard = sync_state.engine.lock().await;
    if let Some(ref engine) = *engine_guard {
        engine.cancel();
        Ok(())
    } else {
        Err("No sync in progress".to_string())
    }
}

// ---------------------------------------------------------------------------
// Background sync scheduler
// ---------------------------------------------------------------------------

/// Start the background sync scheduler that runs periodic sync cycles.
pub fn start_sync_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Wait a bit before the first sync to let the app fully initialize
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        let mut backoff_secs: u64 = 0;

        loop {
            let (sync_enabled, interval_minutes) = {
                let state = app.state::<AppState>();
                let config = state.config.lock().unwrap();
                (
                    config.sync.enabled,
                    config.sync.interval_minutes.max(1) as u64,
                )
            };

            if !sync_enabled {
                // Sync not enabled — check again in 30 seconds
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }

            // Wait for the sync interval (or backoff if there was an error)
            let wait_secs = if backoff_secs > 0 {
                backoff_secs
            } else {
                interval_minutes * 60
            };
            tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;

            // Attempt to sync
            let sync_state = app.state::<SyncState>();

            // Skip if already syncing
            {
                let syncing = sync_state.syncing.lock().await;
                if *syncing {
                    continue;
                }
            }

            let engine_guard = sync_state.engine.lock().await;
            if engine_guard.is_none() {
                // Try to initialize engine from config
                let state = app.state::<AppState>();
                let config = state.config.lock().unwrap().sync.clone();
                if config.enabled && !config.url.is_empty() {
                    if let Ok(client) = create_client(&config) {
                        let engine = SyncEngine::new(
                            client,
                            &config.remote_path,
                            &config.device_id,
                            &config.device_name,
                        );
                        drop(engine_guard);
                        *sync_state.engine.lock().await = Some(engine);
                    }
                }
                continue;
            }

            let engine = engine_guard.as_ref().unwrap();
            *sync_state.syncing.lock().await = true;

            let state = app.state::<AppState>();
            let db_handle: DbHandle = state.db.clone();
            let data_dir = state.data_dir.clone();

            // Check if this is the first sync for this device
            let needs_initial = {
                let device_id = state.config.lock().unwrap().sync.device_id.clone();
                let db_guard = db_handle.lock().unwrap();
                let sync_row =
                    zoro_db::queries::sync::get_or_create_sync_state(&db_guard.conn, &device_id);
                sync_row
                    .map(|r| r.last_sync_time.is_none())
                    .unwrap_or(false)
            };

            let result = if needs_initial {
                info!("Background scheduler: first sync detected — running initial full sync");
                let app_handle = app.clone();
                engine
                    .initial_sync(
                        &db_handle,
                        &data_dir,
                        Some(|progress: SyncProgress| {
                            let _ = app_handle.emit("sync:progress", &progress);
                        }),
                    )
                    .await
            } else {
                let app_handle = app.clone();
                engine
                    .sync(
                        &db_handle,
                        Some(|progress: SyncProgress| {
                            let _ = app_handle.emit("sync:progress", &progress);
                        }),
                    )
                    .await
            };

            // Run L2 file sync after metadata sync
            if result.is_ok() {
                let (remote_path, device_id, max_file_size_mb) = {
                    let config = state.config.lock().unwrap();
                    (
                        config.sync.remote_path.clone(),
                        config.sync.device_id.clone(),
                        config.sync.max_file_size_mb,
                    )
                };
                let max_file_size: u64 = if max_file_size_mb == 0 {
                    0
                } else {
                    max_file_size_mb as u64 * 1_048_576
                };
                let _ = file_sync::sync_all_small_files(
                    engine.client(),
                    &remote_path,
                    &device_id,
                    &data_dir,
                    max_file_size,
                )
                .await;
            }

            drop(engine_guard);
            *sync_state.syncing.lock().await = false;

            match result {
                Ok(()) => {
                    backoff_secs = 0; // Reset backoff on success
                    let _ = app.emit("sync:complete", ());
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    error!("Background sync failed: {}", err_msg);
                    *sync_state.last_error.lock().await = Some(err_msg.clone());
                    let _ = app.emit("sync:error", &err_msg);

                    // Exponential backoff: 60s → 120s → 240s → ... → 1800s (30min)
                    backoff_secs = if backoff_secs == 0 {
                        60
                    } else {
                        (backoff_secs * 2).min(1800)
                    };
                }
            }
        }
    });
}
