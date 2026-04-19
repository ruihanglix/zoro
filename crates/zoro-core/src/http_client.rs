// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::models::ProxyConfig;

/// Build a `reqwest::ClientBuilder` with proxy settings applied.
///
/// When `proxy.enabled` is `true` and `proxy.url` is non-empty, the builder is
/// configured to route all traffic through the given proxy. Otherwise the
/// builder inherits the default system-level proxy (HTTP_PROXY / HTTPS_PROXY
/// env vars).
///
/// Returns a `ClientBuilder` so callers can continue chaining `.timeout()` etc.
pub fn build_http_client(proxy: &ProxyConfig) -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder();

    if proxy.enabled && !proxy.url.is_empty() {
        if let Ok(mut p) = reqwest::Proxy::all(&proxy.url) {
            if !proxy.no_proxy.is_empty() {
                let no_proxy = reqwest::NoProxy::from_string(&proxy.no_proxy);
                p = p.no_proxy(no_proxy);
            }
            builder = builder.proxy(p);
        }
    }

    builder
}
