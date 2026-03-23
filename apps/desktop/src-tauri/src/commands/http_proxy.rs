// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::collections::HashMap;

/// Generic HTTP GET proxy — lets the frontend bypass browser CORS restrictions
/// by routing the request through the Rust backend (reqwest).
///
/// Returns a JSON-serializable response containing status, headers, and body text.
#[derive(Debug, serde::Serialize)]
pub struct ProxyResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[tauri::command]
pub async fn http_proxy_get(
    url: String,
    headers: Option<HashMap<String, String>>,
) -> Result<ProxyResponse, String> {
    tracing::debug!(url = %url, "http_proxy_get: starting request");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| {
            let msg = format!("Failed to build HTTP client: {}", e);
            tracing::warn!(%msg, "http_proxy_get: client build failed");
            msg
        })?;

    let mut req = client.get(&url);

    if let Some(ref h) = headers {
        // Log header keys (not values, to avoid leaking secrets)
        let header_keys: Vec<&String> = h.keys().collect();
        tracing::debug!(?header_keys, "http_proxy_get: sending headers");
        for (k, v) in h {
            req = req.header(k, v);
        }
    }

    let resp = req
        .send()
        .await
        .map_err(|e| {
            let msg = format!("HTTP request failed: {}", e);
            tracing::warn!(url = %url, %msg, "http_proxy_get: request failed");
            msg
        })?;

    let status = resp.status().as_u16();
    tracing::debug!(url = %url, status, "http_proxy_get: got response");

    let mut resp_headers = HashMap::new();
    for (k, v) in resp.headers().iter() {
        if let Ok(val) = v.to_str() {
            resp_headers.insert(k.to_string(), val.to_string());
        }
    }

    let body = resp
        .text()
        .await
        .map_err(|e| {
            let msg = format!("Failed to read response body: {}", e);
            tracing::warn!(url = %url, %msg, "http_proxy_get: body read failed");
            msg
        })?;

    // Log a snippet of the body for debugging (truncate to 512 chars)
    let body_snippet = if body.len() > 512 { &body[..512] } else { &body };
    tracing::debug!(url = %url, status, body_len = body.len(), body_snippet, "http_proxy_get: response body");

    if status < 200 || status >= 300 {
        tracing::warn!(url = %url, status, body = %body_snippet, "http_proxy_get: non-success status");
    }

    Ok(ProxyResponse {
        status,
        headers: resp_headers,
        body,
    })
}
