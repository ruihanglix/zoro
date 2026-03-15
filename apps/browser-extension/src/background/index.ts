// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import { checkConnection } from "./connector";
import type { PaperMetadata } from "../lib/types";

// Track detected papers per tab
const detectedPapers = new Map<number, PaperMetadata>();

// Check connection on startup
checkConnection();

// Listen for paper detection from content scripts
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "PAPER_DETECTED" && sender.tab?.id) {
    detectedPapers.set(sender.tab.id, message.metadata);

    // Update badge to show paper detected
    chrome.action.setBadgeText({
      tabId: sender.tab.id,
      text: "1",
    });
    chrome.action.setBadgeBackgroundColor({
      tabId: sender.tab.id,
      color: "#3b82f6",
    });
  }

  if (message.type === "GET_DETECTED_PAPER" && sender.tab?.id) {
    sendResponse(detectedPapers.get(sender.tab.id) || null);
  }

  return true;
});

// Clean up when tab is closed
chrome.tabs.onRemoved.addListener((tabId) => {
  detectedPapers.delete(tabId);
});

// Clean up when tab navigates
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.url) {
    detectedPapers.delete(tabId);
    chrome.action.setBadgeText({ tabId, text: "" });
  }
});
