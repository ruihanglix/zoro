// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { detectArxiv } from "./detectors/arxiv";
import { detectDoi } from "./detectors/doi";
import { detectGeneric } from "./detectors/generic";
import type { DetectionResult } from "../lib/types";

function detectPaper(): DetectionResult {
  const url = window.location.href;

  // Try detectors in priority order
  const arxivResult = detectArxiv(url, document);
  if (arxivResult.detected) return arxivResult;

  const doiResult = detectDoi(url, document);
  if (doiResult.detected) return doiResult;

  const genericResult = detectGeneric(url, document);
  if (genericResult.detected) return genericResult;

  return { detected: false, source: "none" };
}

// Run detection and send result to background
const result = detectPaper();

if (result.detected && result.metadata) {
  chrome.runtime.sendMessage({
    type: "PAPER_DETECTED",
    metadata: result.metadata,
    source: result.source,
  });
}

// Listen for requests from popup
chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message.type === "GET_PAPER_METADATA") {
    const result = detectPaper();
    sendResponse(result);
  }
  if (message.type === "GET_PAGE_HTML") {
    // Get the full page HTML for saving
    const html = document.documentElement.outerHTML;
    sendResponse({ html });
  }
  return true; // Keep message channel open for async response
});
