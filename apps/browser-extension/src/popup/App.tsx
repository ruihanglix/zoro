// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { useEffect, useState } from "react";
import { ping, saveItem, saveHtml } from "../lib/api";
import type { PaperMetadata } from "../lib/types";
import { initLang, t } from "./i18n";

type Status = "idle" | "detected" | "saving" | "saved" | "error";

export default function App() {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [status, setStatus] = useState<Status>("idle");
  const [metadata, setMetadata] = useState<PaperMetadata | null>(null);
  const [message, setMessage] = useState("");
  const [saveHtmlToo, setSaveHtmlToo] = useState(true);
  const [langReady, setLangReady] = useState(false);

  useEffect(() => {
    // Initialize language
    initLang().then(() => setLangReady(true));

    // Check connection
    ping().then(setConnected);

    // Get detected paper from current tab
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      const tab = tabs[0];
      if (!tab?.id) return;

      chrome.tabs.sendMessage(tab.id, { type: "GET_PAPER_METADATA" }, (response) => {
        if (chrome.runtime.lastError) {
          // Content script not injected on this page
          return;
        }
        if (response?.detected && response.metadata) {
          setMetadata(response.metadata);
          setStatus("detected");
        }
      });
    });
  }, []);

  const handleSave = async () => {
    if (!metadata) return;
    setStatus("saving");
    try {
      const result = await saveItem(metadata);
      if (result.success) {
        setStatus("saved");
        setMessage(t("savedToZoro"));

        // Optionally save HTML content
        if (saveHtmlToo && result.paper_id) {
          chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
            const tab = tabs[0];
            if (!tab?.id) return;
            chrome.tabs.sendMessage(tab.id, { type: "GET_PAGE_HTML" }, async (response) => {
              if (response?.html) {
                await saveHtml(result.paper_id!, response.html);
              }
            });
          });
        }
      } else {
        setStatus("error");
        setMessage(result.message);
      }
    } catch (err) {
      setStatus("error");
      setMessage(`Failed: ${err}`);
    }
  };

  if (!langReady) return null;

  return (
    <div style={{ padding: "16px" }}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "12px" }}>
        <span style={{ fontSize: "18px", fontWeight: "bold" }}>Zoro</span>
        <span
          style={{
            width: "8px", height: "8px", borderRadius: "50%",
            backgroundColor: connected ? "#22c55e" : connected === false ? "#ef4444" : "#9ca3af",
          }}
        />
      </div>

      {/* Connection status */}
      {connected === false && (
        <div style={{ padding: "12px", backgroundColor: "#fef2f2", borderRadius: "6px", marginBottom: "12px", fontSize: "13px", color: "#991b1b" }}>
          {t("appNotRunning")}
        </div>
      )}

      {/* No paper detected */}
      {status === "idle" && connected && (
        <div style={{ padding: "12px", backgroundColor: "#f3f4f6", borderRadius: "6px", fontSize: "13px", color: "#6b7280" }}>
          {t("noPaperDetected")}
        </div>
      )}

      {/* Paper detected */}
      {metadata && status !== "idle" && (
        <div>
          <div style={{ marginBottom: "12px" }}>
            <h3 style={{ fontSize: "14px", fontWeight: 600, lineHeight: 1.4, marginBottom: "4px" }}>
              {metadata.title}
            </h3>
            {metadata.authors.length > 0 && (
              <p style={{ fontSize: "12px", color: "#6b7280" }}>
                {metadata.authors.slice(0, 5).join(", ")}
                {metadata.authors.length > 5 && ` ${t("more", { count: metadata.authors.length - 5 })}`}
              </p>
            )}
            <div style={{ display: "flex", gap: "8px", marginTop: "4px", fontSize: "11px", color: "#9ca3af" }}>
              {metadata.arxiv_id && <span>ArXiv: {metadata.arxiv_id}</span>}
              {metadata.doi && <span>DOI: {metadata.doi}</span>}
              {metadata.pdf_url && <span style={{ color: "#3b82f6" }}>{t("pdfAvailable")}</span>}
              {metadata.html_url && <span style={{ color: "#3b82f6" }}>{t("htmlAvailable")}</span>}
            </div>
          </div>

          {/* Save HTML checkbox */}
          {metadata.html_url && (
            <label style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "13px", marginBottom: "12px", cursor: "pointer" }}>
              <input type="checkbox" checked={saveHtmlToo} onChange={(e) => setSaveHtmlToo(e.target.checked)} />
              {t("alsoSaveHtml")}
            </label>
          )}

          {/* Save button */}
          {status === "detected" && (
            <button
              onClick={handleSave}
              disabled={!connected}
              style={{
                width: "100%", padding: "8px 16px", borderRadius: "6px", border: "none",
                backgroundColor: "#3b82f6", color: "white", fontWeight: 600, fontSize: "14px",
                cursor: connected ? "pointer" : "not-allowed", opacity: connected ? 1 : 0.5,
              }}
            >
              {t("saveToZoro")}
            </button>
          )}

          {status === "saving" && (
            <button disabled style={{ width: "100%", padding: "8px 16px", borderRadius: "6px", border: "none", backgroundColor: "#9ca3af", color: "white", fontWeight: 600, fontSize: "14px" }}>
              {t("saving")}
            </button>
          )}

          {status === "saved" && (
            <div style={{ padding: "8px 16px", borderRadius: "6px", backgroundColor: "#f0fdf4", color: "#166534", textAlign: "center", fontWeight: 600, fontSize: "14px" }}>
              {message}
            </div>
          )}

          {status === "error" && (
            <div style={{ padding: "8px 16px", borderRadius: "6px", backgroundColor: "#fef2f2", color: "#991b1b", textAlign: "center", fontSize: "13px" }}>
              {message}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
