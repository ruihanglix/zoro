// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { PaperMetadata, SaveItemResponse } from "./types";

const DEFAULT_PORT = 23120;

async function getPort(): Promise<number> {
  try {
    const result = await chrome.storage.sync.get("connectorPort");
    return result.connectorPort || DEFAULT_PORT;
  } catch {
    return DEFAULT_PORT;
  }
}

async function getBaseUrl(): Promise<string> {
  const port = await getPort();
  return `http://127.0.0.1:${port}`;
}

export async function ping(): Promise<boolean> {
  try {
    const baseUrl = await getBaseUrl();
    const res = await fetch(`${baseUrl}/connector/ping`, {
      method: "GET",
      headers: { "Content-Type": "application/json" },
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function saveItem(metadata: PaperMetadata): Promise<SaveItemResponse> {
  const baseUrl = await getBaseUrl();
  const res = await fetch(`${baseUrl}/connector/saveItem`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      title: metadata.title,
      authors: metadata.authors,
      url: metadata.url,
      doi: metadata.doi,
      arxiv_id: metadata.arxiv_id,
      pdf_url: metadata.pdf_url,
      html_url: metadata.html_url,
      abstract_text: metadata.abstract_text,
      tags: metadata.tags,
    }),
  });
  return res.json();
}

export async function saveHtml(paperId: string, htmlContent: string): Promise<SaveItemResponse> {
  const baseUrl = await getBaseUrl();
  const res = await fetch(`${baseUrl}/connector/saveHtml`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ paper_id: paperId, html_content: htmlContent }),
  });
  return res.json();
}

export async function getCollections(): Promise<{ id: string; name: string }[]> {
  try {
    const baseUrl = await getBaseUrl();
    const res = await fetch(`${baseUrl}/connector/collections`);
    return res.json();
  } catch {
    return [];
  }
}
