// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use serde::Serialize;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl};

#[derive(Debug, Clone, Serialize)]
struct BrowserNavEvent {
    label: String,
    url: String,
}

#[tauri::command]
pub async fn create_browser_webview(
    app: tauri::AppHandle,
    label: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if app.get_webview(&label).is_some() {
        return Ok(());
    }

    let window = app
        .get_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    let parsed_url: url::Url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;

    let label_for_event = label.clone();
    let app_for_event = app.clone();

    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(parsed_url))
        .on_page_load(move |webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                if let Ok(current_url) = webview.url() {
                    let _ = app_for_event.emit(
                        "browser-navigation",
                        BrowserNavEvent {
                            label: label_for_event.clone(),
                            url: current_url.to_string(),
                        },
                    );
                }
            }
        });

    window
        .add_child(
            builder,
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("Failed to create webview: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn close_browser_webview(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn show_browser_webview(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_browser_webview(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn resize_browser_webview(
    app: tauri::AppHandle,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview
            .set_position(LogicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
        webview
            .set_size(LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_navigate(
    app: tauri::AppHandle,
    label: String,
    url: String,
) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        let parsed: url::Url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
        webview.navigate(parsed).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_go_back(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview
            .eval("window.history.back()")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_go_forward(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview
            .eval("window.history.forward()")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_reload(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if let Some(webview) = app.get_webview(&label) {
        webview.reload().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_get_url(app: tauri::AppHandle, label: String) -> Result<String, String> {
    if let Some(webview) = app.get_webview(&label) {
        let url = webview.url().map_err(|e| e.to_string())?;
        Ok(url.to_string())
    } else {
        Ok(String::new())
    }
}
