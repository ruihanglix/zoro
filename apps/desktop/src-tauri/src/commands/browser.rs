// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl};
use crate::AppState;

/// Global dark mode state shared between the `browser_set_dark_mode` command
/// and the `on_page_load` callback so that newly loaded pages also receive
/// the dark-mode CSS injection.
static DARK_MODE: AtomicBool = AtomicBool::new(false);

/// JS snippet that enables or disables dark-mode on an external web page.
///
/// Uses a **luminance-based** approach (similar to Chrome Auto-Dark / Dark
/// Reader dynamic mode):
/// 1. Walk the DOM and read each element's `getComputedStyle`.
/// 2. Only darken elements whose background is actually *light* (luminance
///    above a threshold).  Already-dark elements are left untouched.
/// 3. Only lighten text that is actually *dark*.
/// 4. A `MutationObserver` handles dynamically-added content.
/// 5. Modified elements are tagged with `data-zd` so we can cleanly undo
///    everything when the user switches back to light mode.
///
/// This avoids the problems of blanket CSS overrides (which destroy sites
/// that already have dark mode) and filter inversion (which distorts images).
fn dark_mode_js(dark: bool) -> String {
    if dark {
        r#"(function(){
if(window.__zoro_dark__)return;
window.__zoro_dark__=true;
var s=document.createElement('style');s.id='zoro-dark-mode';
s.textContent=':root{color-scheme:dark!important}html{scrollbar-color:#555 #1b1b1f}::-webkit-scrollbar{background:#1b1b1f;width:8px}::-webkit-scrollbar-thumb{background:#555;border-radius:4px}::-webkit-scrollbar-thumb:hover{background:#666}img,video{opacity:.85}input::placeholder,textarea::placeholder{color:#888!important}';
(document.head||document.documentElement).appendChild(s);
var SKIP={IMG:1,VIDEO:1,CANVAS:1,PICTURE:1,SVG:1,IFRAME:1,STYLE:1,SCRIPT:1,LINK:1,NOSCRIPT:1,BR:1};
function lum(r,g,b){return .299*r+.587*g+.114*b}
function pc(v){if(!v||v==='transparent'||v==='rgba(0, 0, 0, 0)')return null;var m=v.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);return m?[+m[1],+m[2],+m[3]]:null}
function proc(el){if(!el||el.nodeType!==1||SKIP[el.tagName]||el.id==='zoro-dark-mode')return;
try{var cs=getComputedStyle(el);
var bg=pc(cs.backgroundColor);if(bg){var bl=lum(bg[0],bg[1],bg[2]);
if(bl>150){el.style.setProperty('background-color','#1b1b1f','important');el.setAttribute('data-zd','1')}
else if(bl>100){el.style.setProperty('background-color','#2a2a2e','important');el.setAttribute('data-zd','1')}}
var fg=pc(cs.color);if(fg&&lum(fg[0],fg[1],fg[2])<100){el.style.setProperty('color','#d4d4d8','important');el.setAttribute('data-zd','1')}
var bd=pc(cs.borderTopColor);if(bd&&cs.borderTopStyle!=='none'&&cs.borderTopWidth!=='0px'&&lum(bd[0],bd[1],bd[2])>180){el.style.setProperty('border-color','#3f3f46','important');el.setAttribute('data-zd','1')}}catch(e){}}
function walk(r){proc(r);if(r.querySelectorAll){var a=r.querySelectorAll('*');for(var i=0;i<a.length;i++)proc(a[i])}}
function run(){proc(document.documentElement);walk(document.body||document.documentElement)}
requestAnimationFrame(function(){requestAnimationFrame(run)});
var pend=null;var tgt=document.body||document.documentElement;
var obs=new MutationObserver(function(ms){if(pend)return;var n=[];for(var i=0;i<ms.length;i++){var a=ms[i].addedNodes;for(var j=0;j<a.length;j++)if(a[j].nodeType===1)n.push(a[j])}if(n.length){pend=requestAnimationFrame(function(){for(var i=0;i<n.length;i++)walk(n[i]);pend=null})}});
obs.observe(tgt,{childList:true,subtree:true});window.__zoro_dark_obs__=obs;
})();"#.to_string()
    } else {
        r#"(function(){
var s=document.getElementById('zoro-dark-mode');if(s)s.remove();
if(window.__zoro_dark_obs__){window.__zoro_dark_obs__.disconnect();window.__zoro_dark_obs__=null}
var els=document.querySelectorAll('[data-zd]');for(var i=0;i<els.length;i++){els[i].style.removeProperty('background-color');els[i].style.removeProperty('color');els[i].style.removeProperty('border-color');els[i].removeAttribute('data-zd')}
window.__zoro_dark__=false;
})();"#.to_string()
    }
}

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
    state: State<'_, AppState>,
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

    let mut builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(parsed_url))
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

                // Inject script to extract page title and favicon, with targeted MutationObservers
                let _ = webview.eval(
                    r#"(function(){
                        if(window.__zoro_pageinfo_monitor__)return;
                        window.__zoro_pageinfo_monitor__=true;
                        var lastTitle='',lastFavicon='';
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
                            var t=document.title||'';
                            var f=getFavicon();
                            if(t===lastTitle&&f===lastFavicon)return;
                            lastTitle=t;lastFavicon=f;
                            var a=document.createElement('a');
                            a.href='zoro-page-info://notify?title='+encodeURIComponent(t)+'&favicon='+encodeURIComponent(f);
                            a.click();
                        }
                        send();
                        function observeTitle(el){
                            new MutationObserver(function(){send()}).observe(el,{childList:true,characterData:true,subtree:true});
                        }
                        var titleEl=document.querySelector('title');
                        if(titleEl){
                            observeTitle(titleEl);
                        }
                        function observeFaviconLinks(){
                            document.querySelectorAll('link[rel~="icon"],link[rel="shortcut icon"],link[rel="apple-touch-icon"]').forEach(function(link){
                                if(!link.__zoro_observed){
                                    link.__zoro_observed=true;
                                    new MutationObserver(function(){send()}).observe(link,{attributes:true,attributeFilter:['href']});
                                }
                            });
                        }
                        observeFaviconLinks();
                        new MutationObserver(function(mutations){
                            for(var i=0;i<mutations.length;i++){
                                for(var j=0;j<mutations[i].addedNodes.length;j++){
                                    var node=mutations[i].addedNodes[j];
                                    if(node.nodeName==='TITLE'){
                                        observeTitle(node);send();
                                    } else if(node.nodeName==='LINK'){
                                        var rel=(node.getAttribute('rel')||'').toLowerCase();
                                        if(rel.indexOf('icon')!==-1){
                                            if(!node.__zoro_observed){
                                                node.__zoro_observed=true;
                                                new MutationObserver(function(){send()}).observe(node,{attributes:true,attributeFilter:['href']});
                                            }
                                            send();
                                        }
                                    }
                                }
                            }
                        }).observe(document.head||document.documentElement,{childList:true});
                    })();"#,
                );

                // Inject dark-mode CSS if globally enabled
                if DARK_MODE.load(Ordering::Relaxed) {
                    let _ = webview.eval(&dark_mode_js(true));
                }

                // Inject clipboard helper: tracks the last focused editable
                // element so that Cmd+C/V/X/A operations (dispatched from the
                // native menu via webview.eval) work even after macOS steals
                // focus from the webview.
                let _ = webview.eval(
                    r#"(function(){
                        if(window.__zoro_clipboard__)return;
                        window.__zoro_clipboard__=true;
                        var _el=null,_ss=0,_se=0,_selTxt='';
                        document.addEventListener('focusin',function(e){
                            var t=e.target;
                            if(t&&(t.tagName==='INPUT'||t.tagName==='TEXTAREA'||t.isContentEditable)){
                                _el=t;
                                if(t.tagName==='INPUT'||t.tagName==='TEXTAREA'){
                                    _ss=t.selectionStart||0;_se=t.selectionEnd||0;
                                }
                            }
                        });
                        document.addEventListener('selectionchange',function(){
                            if(_el&&(_el.tagName==='INPUT'||_el.tagName==='TEXTAREA')){
                                _ss=_el.selectionStart||_ss;_se=_el.selectionEnd||_se;
                            }
                            var s=window.getSelection();
                            if(s&&s.toString())_selTxt=s.toString();
                        });
                        // Expose helpers for menu-driven clipboard ops
                        window.__zoro_do_copy=function(){
                            // Try native execCommand first
                            try{if(document.execCommand('copy'))return}catch(e){}
                        };
                        window.__zoro_do_cut=function(){
                            try{document.execCommand('cut')}catch(e){}
                        };
                        window.__zoro_do_paste=function(text){
                            // Re-focus tracked element
                            if(_el){
                                try{_el.focus()}catch(e){}
                                if(_el.tagName==='INPUT'||_el.tagName==='TEXTAREA'){
                                    try{_el.setSelectionRange(_ss,_se)}catch(e){}
                                }
                            }
                            try{document.execCommand('insertText',false,text)}catch(e){}
                        };
                        window.__zoro_do_select_all=function(){
                            if(_el){
                                try{_el.focus()}catch(e){}
                                if(_el.tagName==='INPUT'||_el.tagName==='TEXTAREA'){
                                    try{_el.setSelectionRange(0,_el.value.length)}catch(e){}
                                    return;
                                }
                            }
                            try{document.execCommand('selectAll')}catch(e){}
                        };
                    })();"#,
                );
            }
        });

    // Apply proxy if configured
    {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("Config lock error: {}", e))?;
        if config.proxy.enabled && !config.proxy.url.is_empty() {
            if let Ok(proxy_url) = config.proxy.url.parse::<url::Url>() {
                builder = builder.proxy_url(proxy_url);
            }
        }
    }

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

/// Set dark mode for all browser webviews (everything except the main webview).
/// Also stores the value globally so that newly-loaded pages inherit it.
#[tauri::command]
pub async fn browser_set_dark_mode(app: tauri::AppHandle, dark: bool) -> Result<(), String> {
    DARK_MODE.store(dark, Ordering::Relaxed);
    let js = dark_mode_js(dark);
    for (label, wv) in app.webviews() {
        if label == "main" {
            continue;
        }
        let _ = wv.eval(&js);
    }
    Ok(())
}
