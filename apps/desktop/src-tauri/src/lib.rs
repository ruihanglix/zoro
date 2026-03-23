// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

mod commands;
mod connector;
pub mod log_buffer;
mod storage;
mod subscriptions;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::Manager;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{reload, EnvFilter};
use zoro_core::models::AppConfig;

use log_buffer::{BufferLayer, LogBuffer};

/// Shared state for ACP agent connections (managed separately from AppState
/// because AcpManager needs an async Mutex).
pub struct AcpState {
    pub manager: Arc<tokio::sync::Mutex<zoro_acp::AcpManager>>,
    pub data_dir: PathBuf,
}

/// Reload handle to swap EnvFilter at runtime (debug mode toggle).
pub type FilterReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

pub struct AppState {
    pub db: Arc<Mutex<zoro_db::Database>>,
    pub data_dir: PathBuf,
    pub config: Mutex<AppConfig>,
    /// Cancellation token for the Zotero compat server; None if not running.
    pub zotero_compat_cancel: Mutex<Option<CancellationToken>>,
    /// Last error from the Zotero compat server (e.g. port conflict).
    pub zotero_compat_error: Mutex<Option<String>>,
    pub log_buffer: LogBuffer,
    pub filter_handle: FilterReloadHandle,
    /// Child process handle for the MCP server; None if not running.
    pub mcp_child: Mutex<Option<std::process::Child>>,
    /// Active embedded terminal sessions (PTY).
    pub terminals: Mutex<commands::terminal::TerminalMap>,
    /// Paper IDs currently being translated in background tasks.
    pub active_html_translations: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Whether debug mode is currently enabled (toggled at runtime).
    pub debug_mode: std::sync::atomic::AtomicBool,
}

pub fn make_filter(debug: bool) -> EnvFilter {
    let directive = if debug { "zoro=debug" } else { "zoro=info" };
    EnvFilter::from_default_env().add_directive(directive.parse().unwrap())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Create log buffer (shared with BufferLayer and AppState)
    let log_buffer = log_buffer::new_log_buffer();
    let buffer_layer = BufferLayer::new(log_buffer.clone());

    // Create reloadable env filter for runtime level switching
    let filter = make_filter(false);
    let (filter_layer, filter_handle) = reload::Layer::new(filter);

    // Build layered subscriber: registry + reload(env-filter) + fmt + buffer
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(buffer_layer.clone())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_decorum::init())
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_decorum::WebviewWindowExt;
                let main_window = app.get_webview_window("main").unwrap();
                main_window.create_overlay_titlebar().unwrap();
                main_window.set_traffic_lights_inset(16.0, 14.0).unwrap();

                // Native menu so Cmd+C/V/X/A work on macOS
                let edit_menu = SubmenuBuilder::new(app, "Edit")
                    .item(&PredefinedMenuItem::undo(app, None)?)
                    .item(&PredefinedMenuItem::redo(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::cut(app, None)?)
                    .item(&PredefinedMenuItem::copy(app, None)?)
                    .item(&PredefinedMenuItem::paste(app, None)?)
                    .item(&PredefinedMenuItem::select_all(app, None)?)
                    .build()?;
                let menu = MenuBuilder::new(app).item(&edit_menu).build()?;
                app.set_menu(menu)?;
            }

            // Windows/Linux: remove native title bar and menu bar
            #[cfg(not(target_os = "macos"))]
            {
                let main_window = app.get_webview_window("main").unwrap();
                let _ = main_window.set_decorations(false);
            }

            let data_dir = dirs_data_dir();
            tracing::info!("Data directory: {:?}", data_dir);

            // Initialize storage directories
            storage::init_data_dir(&data_dir).expect("Failed to initialize data directory");

            // Initialize database
            let db_path = data_dir.join("library.db");
            let db = zoro_db::Database::open(&db_path).expect("Failed to open database");

            // Ensure default subscription exists
            ensure_default_subscriptions(&db);

            // Build initial library-index.json
            storage::sync::rebuild_library_index(&db, &data_dir);

            // Load config
            let config = storage::config::load_config(&data_dir);
            let zotero_compat_enabled = config.connector.zotero_compat_enabled;
            let zotero_compat_port = config.connector.zotero_compat_port;
            let native_port = config.connector.port;

            // Give the buffer layer access to the app handle for event emission
            buffer_layer.set_app_handle(app.handle().clone());

            // Enable sync tracking on the database if sync is configured
            let mut db = db;
            if config.sync.enabled && !config.sync.device_id.is_empty() {
                db.enable_sync_tracking(config.sync.device_id.clone());
            }

            // Store state
            app.manage(AppState {
                db: Arc::new(Mutex::new(db)),
                data_dir: data_dir.clone(),
                config: Mutex::new(config.clone()),
                zotero_compat_cancel: Mutex::new(None),
                zotero_compat_error: Mutex::new(None),
                log_buffer: log_buffer.clone(),
                filter_handle: filter_handle.clone(),
                mcp_child: Mutex::new(None),
                terminals: Mutex::new(commands::terminal::new_terminal_map()),
                active_html_translations: Arc::new(Mutex::new(std::collections::HashSet::new())),
                debug_mode: std::sync::atomic::AtomicBool::new(false),
            });

            // Initialize sync state
            app.manage(commands::sync::SyncState::new());

            // Initialize ACP state
            let acp_manager = zoro_acp::AcpManager::new(data_dir.clone());
            app.manage(AcpState {
                manager: Arc::new(tokio::sync::Mutex::new(acp_manager)),
                data_dir: data_dir.clone(),
            });

            // Initialize Chat state
            app.manage(commands::chat::ChatState::new());

            // Initialize Lab (free LLM) state
            let lab_state = commands::lab::LabState::new(&data_dir);
            let lab_enabled = {
                let svc = lab_state.service.blocking_lock();
                svc.is_enabled() && svc.has_configured_providers()
            };
            app.manage(lab_state);

            // Initialize Plugin state
            let plugin_registry = zoro_plugins::PluginRegistry::new(data_dir.clone());
            app.manage(commands::plugins::PluginState {
                registry: Mutex::new(plugin_registry),
            });

            // Start connector HTTP server (Zoro native)
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = connector::start_server(app_handle, native_port).await {
                    tracing::error!("Connector server error: {}", e);
                }
            });

            // Start Zotero compat server if enabled
            if zotero_compat_enabled {
                let app_handle_zotero = app.handle().clone();
                connector::zotero_compat::spawn_zotero_compat_server(
                    app_handle_zotero,
                    zotero_compat_port,
                );
            }

            // Start subscription poller
            let app_handle2 = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                subscriptions::start_poller(app_handle2).await;
            });

            // Start background sync scheduler
            let app_handle3 = app.handle().clone();
            commands::sync::start_sync_scheduler(app_handle3);

            // Start background filesystem scanner (reconcile pdf/html flags)
            let app_handle_fs = app.handle().clone();
            commands::library::start_filesystem_scanner(app_handle_fs);

            // Auto-start MCP server if enabled
            if config.mcp.enabled {
                let app_handle4 = app.handle().clone();
                let _ = commands::mcp::start_mcp_process(&app_handle4);
            }

            // Auto-start Lab proxy if enabled and has configured providers
            if lab_enabled {
                let app_handle_lab = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let lab_state = app_handle_lab.state::<commands::lab::LabState>();
                    // Refresh stale model caches first
                    {
                        let mut svc = lab_state.service.lock().await;
                        svc.refresh_stale_models().await;
                    }
                    // Start the proxy
                    let svc = lab_state.service.lock().await;
                    let proxy_config = svc.to_proxy_config();
                    drop(svc);
                    match zoro_llm_proxy::ProxyServer::start(proxy_config).await {
                        Ok(server) => {
                            tracing::info!(port = server.port(), "Lab LLM proxy auto-started");
                            *lab_state.proxy.lock().await = Some(server);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to auto-start Lab LLM proxy");
                        }
                    }
                });
            }

            Ok(())
        })
        // Re-position macOS traffic lights whenever the window regains focus,
        // because macOS hides them when an overlay-titlebar window is unfocused.
        .on_window_event(|window, event| {
            #[cfg(not(target_os = "macos"))]
            let _ = (&window, &event);

            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::Focused(true) = event {
                use tauri::Manager;
                use tauri_plugin_decorum::WebviewWindowExt;
                if let Some(wv) = window.get_webview_window(window.label()) {
                    let _ = wv.set_traffic_lights_inset(16.0, 14.0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // ACP Agent commands
            commands::acp::acp_list_agents,
            commands::acp::acp_start_session,
            commands::acp::acp_send_prompt,
            commands::acp::acp_cancel_prompt,
            commands::acp::acp_stop_session,
            commands::acp::acp_save_agent_config,
            commands::acp::acp_get_paper_dir,
            commands::acp::acp_set_config_option,
            commands::acp::acp_list_chat_sessions,
            commands::acp::acp_save_chat_session,
            commands::acp::acp_load_chat_session,
            commands::acp::acp_delete_chat_session,
            // Chat commands
            commands::chat::chat_get_config,
            commands::chat::chat_update_config,
            commands::chat::chat_send_message,
            commands::chat::chat_confirm_tool,
            commands::chat::chat_cancel,
            // Library commands
            commands::library::add_paper,
            commands::library::get_paper,
            commands::library::list_papers,
            commands::library::delete_paper,
            commands::library::update_paper_status,
            commands::library::update_paper_rating,
            commands::library::update_paper,
            commands::library::update_paper_authors,
            commands::library::add_attachment_files,
            commands::library::get_paper_pdf_path,
            commands::library::get_paper_html_path,
            commands::library::get_paper_file_path,
            commands::library::import_local_files,
            // Search
            commands::search::search_papers,
            // Collections
            commands::library::create_collection,
            commands::library::list_collections,
            commands::library::delete_collection,
            commands::library::update_collection,
            commands::library::add_paper_to_collection,
            commands::library::remove_paper_from_collection,
            commands::library::get_collections_for_paper,
            commands::library::count_uncategorized_papers,
            commands::library::reorder_collections,
            // Tags
            commands::library::list_tags,
            commands::library::add_tag_to_paper,
            commands::library::remove_tag_from_paper,
            commands::library::delete_tag,
            commands::library::update_tag,
            commands::library::search_tags,
            // Subscriptions
            commands::subscription::list_subscriptions,
            commands::subscription::list_feed_items,
            commands::subscription::add_feed_item_to_library,
            commands::subscription::refresh_subscription,
            commands::subscription::toggle_subscription,
            commands::subscription::fetch_feed_items_by_date,
            commands::subscription::get_latest_feed_date,
            commands::subscription::get_storage_info,
            commands::subscription::clear_feed_cache,
            commands::subscription::change_data_dir,
            commands::subscription::get_subscriptions_config,
            commands::subscription::update_subscriptions_config,
            commands::subscription::fetch_remote_pdf,
            // Zotero Import
            commands::zotero_import::detect_zotero_dir,
            commands::zotero_import::validate_zotero_dir,
            commands::zotero_import::scan_zotero_library,
            commands::zotero_import::import_zotero_library,
            // Import/Export
            commands::import_export::import_bibtex,
            commands::import_export::export_bibtex,
            commands::import_export::import_ris,
            commands::import_export::export_ris,
            commands::import_export::export_annotated_pdf,
            commands::import_export::export_annotated_html,
            commands::import_export::export_pdf,
            commands::import_export::export_html,
            commands::import_export::show_paper_folder,
            commands::import_export::show_attachment_in_folder,
            // Citation
            commands::citation::enrich_paper_metadata,
            commands::citation::search_metadata_candidates,
            commands::citation::apply_metadata_candidate,
            commands::citation::get_formatted_citation,
            commands::citation::get_paper_bibtex,
            // Notes
            commands::notes::add_note,
            commands::notes::list_notes,
            commands::notes::update_note,
            commands::notes::delete_note,
            // Annotations
            commands::annotations::add_annotation,
            commands::annotations::list_annotations,
            commands::annotations::update_annotation,
            commands::annotations::update_annotation_type,
            commands::annotations::delete_annotation,
            // Reader State
            commands::reader_state::get_reader_state,
            commands::reader_state::save_reader_state,
            // Connector
            commands::connector::get_connector_status,
            commands::connector::get_connector_config,
            commands::connector::update_connector_config,
            // Debug
            commands::debug::get_logs,
            commands::debug::set_debug_mode,
            commands::debug::clear_logs,
            // Sync
            commands::sync::test_webdav_connection,
            commands::sync::save_sync_config,
            commands::sync::trigger_sync,
            commands::sync::get_sync_config,
            commands::sync::get_sync_status,
            commands::sync::download_paper_file,
            commands::sync::cancel_sync,
            // MCP Server
            commands::mcp::get_mcp_status,
            commands::mcp::update_mcp_config,
            commands::mcp::start_mcp_server,
            commands::mcp::stop_mcp_server,
            commands::mcp::restart_mcp_server,
            // Translation / AI
            commands::translation::get_translations,
            commands::translation::get_translations_batch,
            commands::translation::translate_fields,
            commands::translation::delete_translations,
            commands::translation::get_ai_config,
            commands::translation::update_ai_config,
            commands::translation::test_ai_connection,
            commands::translation::reset_translation_prompts,
            commands::translation::translate_selection,
            commands::translation::translate_pdf,
            commands::translation::test_babeldoc,
            // Glossary
            commands::translation::get_glossary,
            commands::translation::add_glossary_term,
            commands::translation::update_glossary_term,
            commands::translation::promote_glossary_term,
            commands::translation::delete_glossary_term,
            commands::translation::clear_glossary,
            // Browser (native webviews)
            commands::browser::create_browser_webview,
            commands::browser::close_browser_webview,
            commands::browser::show_browser_webview,
            commands::browser::hide_browser_webview,
            commands::browser::resize_browser_webview,
            commands::browser::browser_navigate,
            commands::browser::browser_go_back,
            commands::browser::browser_go_forward,
            commands::browser::browser_reload,
            commands::browser::browser_get_url,
            // Papers.cool
            commands::papers_cool::papers_cool_index,
            commands::papers_cool::papers_cool_browse_arxiv,
            commands::papers_cool::papers_cool_browse_venue,
            commands::papers_cool::papers_cool_search,
            // arXiv HTML
            commands::arxiv::fetch_arxiv_html,
            commands::arxiv::clean_paper_html,
            commands::arxiv::fix_paper_html_style,
            commands::arxiv::translate_paper_html,
            commands::arxiv::get_active_html_translations,
            commands::arxiv::save_html_translation_edit,
            commands::arxiv::count_html_untranslated,
            // Terminal
            commands::terminal::spawn_terminal,
            commands::terminal::write_terminal,
            commands::terminal::resize_terminal,
            commands::terminal::close_terminal,
            commands::terminal::get_terminal_history,
            // Plugins
            commands::plugins::list_plugins,
            commands::plugins::install_plugin_from_file,
            commands::plugins::uninstall_plugin,
            commands::plugins::toggle_plugin,
            commands::plugins::load_dev_plugin,
            commands::plugins::unload_dev_plugin,
            commands::plugins::reload_dev_plugin,
            commands::plugins::plugin_storage_get,
            commands::plugins::plugin_storage_set,
            commands::plugins::plugin_storage_delete,
            // Plugin AI
            commands::plugins::plugin_ai_chat,
            commands::plugins::plugin_ai_chat_stream,
            commands::plugins::plugin_ai_get_models,
            // HTTP Proxy (bypass browser CORS)
            commands::http_proxy::http_proxy_get,
            // Lab (free LLM proxy)
            commands::lab::lab_get_config,
            commands::lab::lab_update_config,
            commands::lab::lab_list_providers,
            commands::lab::lab_set_provider_key,
            commands::lab::lab_list_models,
            commands::lab::lab_toggle_model,
            commands::lab::lab_refresh_models,
            commands::lab::lab_set_strategy,
            commands::lab::lab_start_proxy,
            commands::lab::lab_stop_proxy,
            commands::lab::lab_get_proxy_status,
            commands::lab::lab_reload_proxy,
            commands::lab::lab_set_enabled,
            commands::lab::lab_set_proxy_port,
            commands::lab::lab_set_lan_access,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn dirs_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let default_dir = PathBuf::from(&home).join(".zoro");

    // Check if config.toml in the default directory points elsewhere
    let config = storage::config::load_config(&default_dir);
    let configured = config.general.data_dir.clone();
    if !configured.is_empty() && configured != "~/.zoro" {
        // Expand ~ in the configured path
        let expanded = if let Some(stripped) = configured.strip_prefix("~/") {
            PathBuf::from(&home).join(stripped)
        } else {
            PathBuf::from(&configured)
        };
        if expanded.exists() {
            return expanded;
        }
    }

    default_dir
}

fn ensure_default_subscriptions(db: &zoro_db::Database) {
    use zoro_db::queries::subscriptions;
    let subs = subscriptions::list_subscriptions(&db.conn).unwrap_or_default();
    if !subs.iter().any(|s| s.source_type == "huggingface-daily") {
        let _ = subscriptions::create_subscription(
            &db.conn,
            "huggingface-daily",
            "HuggingFace Daily Papers",
            Some(r#"{"auto_download_pdf": false}"#),
            60,
        );
        tracing::info!("Created default HuggingFace Daily Papers subscription");
    }
    if !subs.iter().any(|s| s.source_type == "papers-cool") {
        let _ = subscriptions::create_subscription(&db.conn, "papers-cool", "Papers.cool", None, 0);
        tracing::info!("Created default Papers.cool subscription");
    }
}
