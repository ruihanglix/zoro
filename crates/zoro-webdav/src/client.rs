// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use base64::Engine;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::{Client, Method, StatusCode};
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};
use url::Url;

use crate::error::WebDavError;
use crate::rate_limiter::RateLimiter;
use crate::types::DavResource;

/// WebDAV client that wraps HTTP operations with authentication,
/// rate limiting, and exponential backoff retry.
#[derive(Clone)]
pub struct WebDavClient {
    http: Client,
    base_url: Url,
    auth_header: String,
    rate_limiter: RateLimiter,
    max_retries: u32,
}

impl WebDavClient {
    /// Create a new WebDAV client.
    ///
    /// - `url`: base URL of the WebDAV server (e.g. "https://dav.jianguoyun.com/dav/")
    /// - `username`: WebDAV username
    /// - `password`: WebDAV password or app-specific password
    /// - `rate_limiter`: optional rate limiter (defaults to moderate limits)
    pub fn new(
        url: &str,
        username: &str,
        password: &str,
        rate_limiter: Option<RateLimiter>,
    ) -> Result<Self, WebDavError> {
        let base_url = Url::parse(url)?;
        let credentials = format!("{}:{}", username, password);
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(credentials)
        );

        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            http,
            base_url,
            auth_header,
            rate_limiter: rate_limiter.unwrap_or_else(RateLimiter::unlimited),
            max_retries: 3,
        })
    }

    /// Resolve a relative path against the base URL.
    fn resolve_url(&self, path: &str) -> Result<Url, WebDavError> {
        let normalized = path.trim_start_matches('/');
        self.base_url.join(normalized).map_err(|e| e.into())
    }

    /// Execute an HTTP request with rate limiting and exponential backoff retry.
    async fn execute_with_retry(
        &self,
        method: Method,
        url: &Url,
        body: Option<Vec<u8>>,
        extra_headers: Option<Vec<(&str, &str)>>,
    ) -> Result<reqwest::Response, WebDavError> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                warn!(
                    attempt,
                    backoff_ms = backoff.as_millis(),
                    "Retrying WebDAV request"
                );
                tokio::time::sleep(backoff).await;
            }

            self.rate_limiter.acquire().await;

            let mut req = self
                .http
                .request(method.clone(), url.clone())
                .header("Authorization", &self.auth_header);

            if let Some(ref headers) = extra_headers {
                for (key, value) in headers {
                    req = req.header(*key, *value);
                }
            }

            if let Some(ref body_bytes) = body {
                req = req.body(body_bytes.clone());
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() || status == StatusCode::MULTI_STATUS {
                        return Ok(resp);
                    }
                    match status {
                        StatusCode::UNAUTHORIZED => return Err(WebDavError::AuthenticationFailed),
                        StatusCode::FORBIDDEN => {
                            return Err(WebDavError::PermissionDenied(url.to_string()))
                        }
                        StatusCode::NOT_FOUND => {
                            return Err(WebDavError::NotFound(url.to_string()))
                        }
                        StatusCode::CONFLICT => return Err(WebDavError::Conflict(url.to_string())),
                        StatusCode::TOO_MANY_REQUESTS => {
                            let retry_after = resp
                                .headers()
                                .get("retry-after")
                                .and_then(|v| v.to_str().ok())
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(30);
                            last_error = Some(WebDavError::RateLimited {
                                retry_after_secs: retry_after,
                            });
                            tokio::time::sleep(Duration::from_secs(retry_after)).await;
                            continue;
                        }
                        s if s.is_server_error() => {
                            last_error = Some(WebDavError::ServerError {
                                status: s.as_u16(),
                                message: resp.text().await.unwrap_or_default(),
                            });
                            continue;
                        }
                        _ => {
                            return Err(WebDavError::ServerError {
                                status: status.as_u16(),
                                message: resp.text().await.unwrap_or_default(),
                            });
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(WebDavError::HttpError(e));
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or(WebDavError::Other("Max retries exceeded".to_string())))
    }

    /// Test the WebDAV connection by performing a PROPFIND on the base URL.
    pub async fn test_connection(&self) -> Result<(), WebDavError> {
        info!(url = %self.base_url, "Testing WebDAV connection");
        let url = self.base_url.clone();
        self.execute_with_retry(
            Method::from_bytes(b"PROPFIND").unwrap(),
            &url,
            None,
            Some(vec![("Depth", "0")]),
        )
        .await?;
        info!("WebDAV connection test successful");
        Ok(())
    }

    /// Create a directory (collection) on the WebDAV server.
    /// Creates parent directories as needed.
    pub async fn mkcol(&self, path: &str) -> Result<(), WebDavError> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut current = String::new();

        for part in parts {
            current = format!("{}/{}", current, part);
            let url = self.resolve_url(&format!("{}/", current))?;
            debug!(path = %current, "Creating directory");

            match self
                .execute_with_retry(Method::from_bytes(b"MKCOL").unwrap(), &url, None, None)
                .await
            {
                Ok(_) => {}
                // Directory already exists - that's fine
                Err(WebDavError::Conflict(_)) => {}
                Err(WebDavError::ServerError { status: 405, .. }) => {
                    // Method not allowed usually means collection already exists
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Upload data to a file on the WebDAV server.
    pub async fn put(&self, path: &str, data: Vec<u8>) -> Result<(), WebDavError> {
        let url = self.resolve_url(path)?;
        debug!(path, size = data.len(), "Uploading file");
        self.execute_with_retry(Method::PUT, &url, Some(data), None)
            .await?;
        Ok(())
    }

    /// Upload data with a specific content type.
    pub async fn put_with_content_type(
        &self,
        path: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<(), WebDavError> {
        let url = self.resolve_url(path)?;
        debug!(path, size = data.len(), content_type, "Uploading file");
        self.execute_with_retry(
            Method::PUT,
            &url,
            Some(data),
            Some(vec![("Content-Type", content_type)]),
        )
        .await?;
        Ok(())
    }

    /// Download a file from the WebDAV server as bytes.
    pub async fn get(&self, path: &str) -> Result<Vec<u8>, WebDavError> {
        let url = self.resolve_url(path)?;
        debug!(path, "Downloading file");
        let resp = self
            .execute_with_retry(Method::GET, &url, None, None)
            .await?;
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Download a file to a local path, reporting progress via callback.
    pub async fn get_to_file<F>(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress_cb: Option<F>,
    ) -> Result<u64, WebDavError>
    where
        F: Fn(u64, Option<u64>) + Send,
    {
        let url = self.resolve_url(remote_path)?;
        self.rate_limiter.acquire().await;

        let resp = self
            .http
            .get(url)
            .header("Authorization", &self.auth_header)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(WebDavError::ServerError {
                status: resp.status().as_u16(),
                message: "Download failed".to_string(),
            });
        }

        let total_size = resp.content_length();

        // Ensure parent directory exists
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(local_path).await?;
        let mut downloaded: u64 = 0;

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if let Some(ref cb) = progress_cb {
                cb(downloaded, total_size);
            }
        }

        file.flush().await?;
        debug!(
            remote_path,
            local = %local_path.display(),
            bytes = downloaded,
            "File download complete"
        );
        Ok(downloaded)
    }

    /// Delete a resource on the WebDAV server.
    pub async fn delete(&self, path: &str) -> Result<(), WebDavError> {
        let url = self.resolve_url(path)?;
        debug!(path, "Deleting resource");
        self.execute_with_retry(Method::DELETE, &url, None, None)
            .await?;
        Ok(())
    }

    /// Check if a resource exists (HEAD request).
    pub async fn exists(&self, path: &str) -> Result<bool, WebDavError> {
        let url = self.resolve_url(path)?;
        match self
            .execute_with_retry(Method::HEAD, &url, None, None)
            .await
        {
            Ok(_) => Ok(true),
            Err(WebDavError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// List directory contents using PROPFIND (Depth: 1).
    pub async fn list(&self, path: &str) -> Result<Vec<DavResource>, WebDavError> {
        let url = self.resolve_url(&format!("{}/", path.trim_end_matches('/')))?;
        let propfind_body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:resourcetype/>
    <D:getcontentlength/>
    <D:getlastmodified/>
    <D:getetag/>
    <D:getcontenttype/>
  </D:prop>
</D:propfind>"#;

        let resp = self
            .execute_with_retry(
                Method::from_bytes(b"PROPFIND").unwrap(),
                &url,
                Some(propfind_body.as_bytes().to_vec()),
                Some(vec![
                    ("Depth", "1"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ]),
            )
            .await?;

        let body = resp.text().await?;
        parse_propfind_response(&body)
    }

    /// Initialize the remote directory structure for Zoro sync.
    pub async fn init_remote_dirs(&self, remote_root: &str) -> Result<(), WebDavError> {
        info!(remote_root, "Initializing remote directory structure");
        let root = remote_root.trim_end_matches('/');
        self.mkcol(&format!("{}/zoro", root)).await?;
        self.mkcol(&format!("{}/zoro/sync", root)).await?;
        self.mkcol(&format!("{}/zoro/sync/changelog", root)).await?;
        self.mkcol(&format!("{}/zoro/library", root)).await?;
        self.mkcol(&format!("{}/zoro/library/papers", root)).await?;
        info!("Remote directory structure initialized");
        Ok(())
    }

    /// Upload a file from local filesystem to WebDAV.
    pub async fn put_file(&self, remote_path: &str, local_path: &Path) -> Result<(), WebDavError> {
        let data = tokio::fs::read(local_path).await?;
        self.put(remote_path, data).await
    }

    /// Download a JSON file and deserialize it.
    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, WebDavError> {
        let data = self.get(path).await?;
        let value = serde_json::from_slice(&data)?;
        Ok(value)
    }

    /// Serialize and upload a JSON file.
    pub async fn put_json<T: serde::Serialize>(
        &self,
        path: &str,
        value: &T,
    ) -> Result<(), WebDavError> {
        let data = serde_json::to_vec_pretty(value)?;
        self.put_with_content_type(path, data, "application/json")
            .await
    }
}

/// Parse a PROPFIND multistatus XML response into DavResource entries.
fn parse_propfind_response(xml: &str) -> Result<Vec<DavResource>, WebDavError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut resources = Vec::new();
    let mut current_href: Option<String> = None;
    let mut is_collection = false;
    let mut content_length: Option<u64> = None;
    let mut last_modified: Option<String> = None;
    let mut etag: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut in_response = false;
    let mut current_tag = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                current_tag = name.to_string();

                match name {
                    "response" => {
                        in_response = true;
                        current_href = None;
                        is_collection = false;
                        content_length = None;
                        last_modified = None;
                        etag = None;
                        content_type = None;
                    }
                    "collection" if in_response => {
                        is_collection = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                if name == "response" && in_response {
                    if let Some(href) = current_href.take() {
                        resources.push(DavResource {
                            href,
                            is_collection,
                            content_length,
                            last_modified: last_modified.take(),
                            etag: etag.take(),
                            content_type: content_type.take(),
                        });
                    }
                    in_response = false;
                }
                current_tag.clear();
            }
            Ok(Event::Text(ref e)) if in_response => {
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "href" => current_href = Some(text),
                    "getcontentlength" => content_length = text.parse().ok(),
                    "getlastmodified" => last_modified = Some(text),
                    "getetag" => etag = Some(text),
                    "getcontenttype" => content_type = Some(text),
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) if in_response => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                if name == "collection" {
                    is_collection = true;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(WebDavError::XmlParseError(e.to_string())),
            _ => {}
        }
    }

    Ok(resources)
}
