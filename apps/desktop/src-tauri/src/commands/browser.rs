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

#[derive(Debug, Clone, Serialize)]
struct BrowserPageInfoEvent {
    label: String,
    title: String,
    favicon: String,
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

    let label_for_pageinfo = label.clone();
    let app_for_pageinfo = app.clone();

    let label_for_nav = label.clone();
    let app_for_nav = app.clone();

    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(parsed_url))
        .on_navigation(move |url| {
            // Intercept SPA URL change notifications from injected JS
            if url.scheme() == "zoro-url-change" {
                if let Some((_, real_url)) = url.query_pairs().find(|(k, _)| k == "url") {
                    let _ = app_for_nav.emit(
                        "browser-navigation",
                        BrowserNavEvent {
                            label: label_for_nav.clone(),
                            url: real_url.to_string(),
                        },
                    );
                }
                return false; // Block the fake navigation
            }
            // Intercept page info notifications (title + favicon)
            if url.scheme() == "zoro-page-info" {
                let title = url
                    .query_pairs()
                    .find(|(k, _)| k == "title")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                let favicon = url
                    .query_pairs()
                    .find(|(k, _)| k == "favicon")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                let _ = app_for_pageinfo.emit(
                    "browser-page-info",
                    BrowserPageInfoEvent {
                        label: label_for_pageinfo.clone(),
                        title,
                        favicon,
                    },
                );
                return false; // Block the fake navigation
            }
            true
        })
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

                // Inject script to monitor SPA URL changes (pushState/replaceState)
                let _ = webview.eval(
                    r#"(function(){
                        if(window.__zoro_url_monitor__)return;
                        window.__zoro_url_monitor__=true;
                        var last=location.href;
                        function notify(){
                            var cur=location.href;
                            if(cur!==last){
                                last=cur;
                                var a=document.createElement('a');
                                a.href='zoro-url-change://notify?url='+encodeURIComponent(cur);
                                a.click();
                            }
                        }
                        var origPush=history.pushState;
                        history.pushState=function(){
                            origPush.apply(this,arguments);
                            notify();
                        };
                        var origReplace=history.replaceState;
                        history.replaceState=function(){
                            origReplace.apply(this,arguments);
                            notify();
                        };
                        window.addEventListener('popstate',function(){notify()});
                    })();"#,
                );

                // Inject script to extract page title and favicon, with MutationObserver for dynamic title changes
                let _ = webview.eval(
                    r#"(function(){
                        if(window.__zoro_pageinfo_monitor__)return;
                        window.__zoro_pageinfo_monitor__=true;
                        function getFavicon(){
                            var icons=document.querySelectorAll('link[rel~="icon"],link[rel="shortcut icon"],link[rel="apple-touch-icon"]');
                            for(var i=icons.length-1;i>=0;i--){
                                var href=icons[i].getAttribute('href');
                                if(href){
                                    try{return new URL(href,location.href).href}catch(e){}
                                }
                            }
                            return location.origin+'/favicon.ico';
                        }
                        function send(){
                            var a=document.createElement('a');
                            a.href='zoro-page-info://notify?title='+encodeURIComponent(document.title||'')+'&favicon='+encodeURIComponent(getFavicon());
                            a.click();
                        }
                        send();
                        var titleEl=document.querySelector('title');
                        if(titleEl){
                            new MutationObserver(function(){send()}).observe(titleEl,{childList:true,characterData:true,subtree:true});
                        } else {
                            new MutationObserver(function(m,obs){
                                var t=document.querySelector('title');
                                if(t){
                                    obs.disconnect();
                                    send();
                                    new MutationObserver(function(){send()}).observe(t,{childList:true,characterData:true,subtree:true});
                                }
                            }).observe(document.head||document.documentElement,{childList:true,subtree:true});
                        }
                        new MutationObserver(function(){send()}).observe(document.head||document.documentElement,{childList:true,subtree:true});
                    })();"#,
                );
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
        // Belt-and-suspenders: also move off-screen in case hide() is unreliable
        // on some platforms (e.g. macOS child webviews).
        let _ = webview.set_position(LogicalPosition::new(-10000.0, -10000.0));
        let _ = webview.set_size(LogicalSize::new(1.0, 1.0));
    }
    Ok(())
}

/// Hide ALL browser child webviews (everything except the main webview).
/// Called on tab switch to guarantee a clean slate.
#[tauri::command]
pub async fn hide_all_browser_webviews(app: tauri::AppHandle) -> Result<(), String> {
    let webviews = app.webviews();
    for (label, wv) in webviews {
        // Skip the main app webview
        if label == "main" {
            continue;
        }
        let _ = wv.hide();
        let _ = wv.set_position(LogicalPosition::new(-10000.0, -10000.0));
        let _ = wv.set_size(LogicalSize::new(1.0, 1.0));
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
