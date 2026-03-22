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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let mut req = client.get(&url);

    if let Some(h) = headers {
        for (k, v) in h {
            req = req.header(&k, &v);
        }
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = resp.status().as_u16();

    let mut resp_headers = HashMap::new();
    for (k, v) in resp.headers().iter() {
        if let Ok(val) = v.to_str() {
            resp_headers.insert(k.to_string(), val.to_string());
        }
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    Ok(ProxyResponse {
        status,
        headers: resp_headers,
        body,
    })
}
