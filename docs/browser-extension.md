# Browser Extension

The Zoro browser extension is a Chrome extension (Manifest V3) that detects academic papers on web pages and saves them to the Zoro desktop app via the local connector HTTP server.

## Architecture Overview

```
+-------------------------------------------------------------------+
|  Chrome Browser                                                     |
|                                                                     |
|  +------------------+    +------------------+    +--------------+  |
|  | Content Scripts  |    | Service Worker   |    | Popup UI     |  |
|  | (per tab)        |--->| (background.js)  |--->| (popup HTML) |  |
|  |                  |    |                  |    |              |  |
|  | - arxiv.ts       |    | - Message router |    | - Show paper |  |
|  | - doi.ts         |    | - API calls to   |    | - Edit meta  |  |
|  | - generic.ts     |    |   connector      |    | - Save btn   |  |
|  +------------------+    +------------------+    +--------------+  |
|                                   |                                 |
+-------------------------------------------------------------------+
                                    | HTTP (localhost:23120)
                                    v
                          +-------------------+
                          | Zoro Desktop  |
                          | Connector Server  |
                          +-------------------+
```

The extension follows Chrome Manifest V3 architecture:

- **Content scripts** run on matching pages and detect paper metadata
- **Service worker** (`background.js`) handles inter-component messaging and HTTP requests
- **Popup UI** provides the user interface for reviewing and saving papers

## Manifest V3 Configuration

```json
{
  "manifest_version": 3,
  "name": "Zoro",
  "version": "0.1.0",
  "description": "Save papers to Zoro - AI-native literature manager",
  "permissions": ["activeTab", "storage"],
  "host_permissions": ["http://127.0.0.1:23120/*", "http://127.0.0.1:23119/*"],
  "action": {
    "default_popup": "src/popup/index.html"
  },
  "background": {
    "service_worker": "background.js",
    "type": "module"
  },
  "content_scripts": [
    {
      "matches": [
        "*://arxiv.org/*",
        "*://www.arxiv.org/*",
        "*://scholar.google.com/*",
        "*://doi.org/*"
      ],
      "js": ["content.js"],
      "run_at": "document_idle"
    }
  ]
}
```

Key details:
- **`permissions`**: `activeTab` for accessing the current tab, `storage` for persisting settings
- **`host_permissions`**: Access to `127.0.0.1:23120` (connector) and `23119` (fallback)
- **Content scripts** match ArXiv, Google Scholar, and DOI resolution pages
- **`run_at: document_idle`** ensures the page DOM is fully loaded before detection

## Paper Detector System

The extension includes three detectors, applied in priority order:

| Detector | File | Sites | Method |
|---|---|---|---|
| ArXiv | `detectors/arxiv.ts` | `arxiv.org/abs/*`, `arxiv.org/pdf/*`, `arxiv.org/html/*` | URL pattern + page DOM |
| DOI | `detectors/doi.ts` | `doi.org/*`, any page with DOI meta tags | URL pattern + meta tags |
| Generic | `detectors/generic.ts` | Any page with `citation_*` meta tags | Google Scholar / Highwire Press meta tags |

### Shared Types

All detectors return a `DetectionResult` (from `src/lib/types.ts`):

```typescript
export interface PaperMetadata {
  title: string;
  authors: string[];
  url: string;
  doi?: string;
  arxiv_id?: string;
  pdf_url?: string;
  html_url?: string;
  abstract_text?: string;
  tags?: string[];
}

export interface DetectionResult {
  detected: boolean;
  metadata?: PaperMetadata;
  source: string;
}
```

### ArXiv Detector

Detects papers on `arxiv.org` by matching URL patterns and extracting metadata from the page DOM:

```typescript
export function detectArxiv(url: string, doc: Document): DetectionResult {
  // Match arxiv.org/abs/XXXX.XXXXX or arxiv.org/pdf/XXXX.XXXXX
  const absMatch = url.match(/arxiv\.org\/abs\/(\d+\.\d+)(v\d+)?/);
  const pdfMatch = url.match(/arxiv\.org\/pdf\/(\d+\.\d+)(v\d+)?/);
  const htmlMatch = url.match(/arxiv\.org\/html\/(\d+\.\d+)(v\d+)?/);

  const arxivId = absMatch?.[1] || pdfMatch?.[1] || htmlMatch?.[1];
  if (!arxivId) {
    return { detected: false, source: "arxiv" };
  }

  // Extract metadata from the page
  const title =
    doc.querySelector("h1.title")?.textContent?.replace(/^Title:\s*/, "").trim() ||
    doc.querySelector('meta[name="citation_title"]')?.getAttribute("content") ||
    doc.title.replace(" - arXiv", "").trim() || "";

  const authorElements = doc.querySelectorAll(".authors a, meta[name='citation_author']");
  const authors: string[] = [];
  authorElements.forEach((el) => {
    const name = el.getAttribute("content") || el.textContent?.trim();
    if (name && !authors.includes(name)) {
      authors.push(name);
    }
  });

  // ... abstract, DOI extraction ...

  return {
    detected: true,
    source: "arxiv",
    metadata: {
      title,
      authors,
      url: `https://arxiv.org/abs/${arxivId}`,
      arxiv_id: arxivId,
      doi,
      pdf_url: `https://arxiv.org/pdf/${arxivId}`,
      html_url: `https://arxiv.org/html/${arxivId}`,
      abstract_text,
    },
  };
}
```

### DOI Detector

Detects papers via DOI, checking both the URL and meta tags:

```typescript
export function detectDoi(url: string, doc: Document): DetectionResult {
  // Look for DOI in URL
  const doiUrlMatch = url.match(/doi\.org\/(10\.\d{4,}\/[^\s]+)/);

  // Look for DOI in meta tags
  const doiMeta =
    doc.querySelector('meta[name="citation_doi"]')?.getAttribute("content") ||
    doc.querySelector('meta[name="DC.identifier"]')?.getAttribute("content") ||
    doc.querySelector('meta[name="dc.identifier"]')?.getAttribute("content");

  const doi = doiUrlMatch?.[1] || (doiMeta?.startsWith("10.") ? doiMeta : undefined);
  if (!doi) {
    return { detected: false, source: "doi" };
  }

  // Extract title, authors, abstract from meta tags ...
  return { detected: true, source: "doi", metadata: { ... } };
}
```

### Generic Detector

Falls back to Google Scholar / Highwire Press `citation_*` meta tags, which are widely used by academic publishers:

```typescript
export function detectGeneric(url: string, doc: Document): DetectionResult {
  const title = doc.querySelector('meta[name="citation_title"]')?.getAttribute("content");
  if (!title) {
    return { detected: false, source: "generic" };
  }

  const authorMetas = doc.querySelectorAll('meta[name="citation_author"]');
  const doi = doc.querySelector('meta[name="citation_doi"]')?.getAttribute("content");
  const pdf_url = doc.querySelector('meta[name="citation_pdf_url"]')?.getAttribute("content");
  const abstract_text = doc.querySelector('meta[name="citation_abstract"]')?.getAttribute("content");

  return {
    detected: true,
    source: "generic",
    metadata: { title, authors, url, doi, pdf_url, abstract_text },
  };
}
```

## How to Add a New Site Detector

### 1. Create the Detector File

Create a new file in `apps/browser-extension/src/content/detectors/`, e.g., `semantic_scholar.ts`:

```typescript
import type { DetectionResult } from "../../lib/types";

export function detectSemanticScholar(url: string, doc: Document): DetectionResult {
  // Check if we're on a Semantic Scholar paper page
  const match = url.match(/semanticscholar\.org\/paper\/([a-f0-9]+)/i);
  if (!match) {
    return { detected: false, source: "semantic-scholar" };
  }

  // Extract metadata from the page DOM
  const title = doc.querySelector('h1[data-test-id="paper-detail-title"]')?.textContent?.trim() || "";
  
  // ... extract authors, abstract, etc. ...

  return {
    detected: true,
    source: "semantic-scholar",
    metadata: {
      title,
      authors,
      url,
      doi,
      abstract_text,
    },
  };
}
```

### 2. Register the Detector

Import and call your detector in the content script detection pipeline. Detectors are tried in order -- more specific detectors should come before more general ones.

### 3. Add Content Script Matching

If your target site is not already covered by the content script `matches` in `manifest.json`, add it:

```json
{
  "content_scripts": [
    {
      "matches": [
        "*://arxiv.org/*",
        "*://www.arxiv.org/*",
        "*://scholar.google.com/*",
        "*://doi.org/*",
        "*://www.semanticscholar.org/*"
      ],
      "js": ["content.js"],
      "run_at": "document_idle"
    }
  ]
}
```

### 4. Test

1. Build the extension: `pnpm --filter @zoro/browser-extension build`
2. Load unpacked in Chrome (see [Building for Development](#building-for-development))
3. Navigate to a paper page on the new site
4. Click the extension popup to verify detection

## Connector API Protocol

The extension communicates with the desktop app via HTTP requests to the connector server at `http://127.0.0.1:23120`.

### `GET /connector/ping`

Health check endpoint. Returns version and application name.

**Response:**

```json
{
  "version": "0.1.0",
  "name": "Zoro"
}
```

### `POST /connector/saveItem`

Save a detected paper to the library.

**Request:**

```json
{
  "title": "Attention Is All You Need",
  "authors": ["Ashish Vaswani", "Noam Shazeer", "Niki Parmar"],
  "url": "https://arxiv.org/abs/1706.03762",
  "doi": "10.48550/arXiv.1706.03762",
  "arxiv_id": "1706.03762",
  "pdf_url": "https://arxiv.org/pdf/1706.03762",
  "html_url": "https://arxiv.org/html/1706.03762",
  "abstract_text": "The dominant sequence transduction models...",
  "tags": ["transformer", "attention"]
}
```

All fields except `title` are optional.

**Response (success):**

```json
{
  "success": true,
  "paper_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "Paper saved successfully"
}
```

**Response (failure):**

```json
{
  "success": false,
  "paper_id": null,
  "message": "Failed to save paper: duplicate slug"
}
```

### `POST /connector/saveHtml`

Save an HTML snapshot of a paper page. Called after `saveItem` to capture the full-text HTML version.

**Request:**

```json
{
  "paper_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "html_content": "<!DOCTYPE html><html>..."
}
```

**Response:**

```json
{
  "success": true,
  "paper_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "HTML saved successfully"
}
```

The HTML is saved to `~/.zoro/library/papers/{slug}/abs.html`.

### `GET /connector/status`

Check the current status of the connector server.

**Response:**

```json
{
  "status": "ready",
  "current_save": null
}
```

### `GET /connector/collections`

List all collections in the library (for the extension to offer collection selection).

**Response:**

```json
[
  { "id": "uuid-1", "name": "Machine Learning" },
  { "id": "uuid-2", "name": "NLP" }
]
```

## Building for Development

### Build the Extension

```bash
pnpm --filter @zoro/browser-extension build
```

This outputs the built extension to `apps/browser-extension/dist/`.

### Load in Chrome

1. Open Chrome and navigate to `chrome://extensions/`
2. Enable **Developer mode** (toggle in the top-right)
3. Click **Load unpacked**
4. Select the `apps/browser-extension/dist/` directory
5. The Zoro extension icon appears in the toolbar

### Development Workflow

1. Make changes to the extension source
2. Run `pnpm --filter @zoro/browser-extension build`
3. In `chrome://extensions/`, click the reload button on the Zoro extension
4. Test on a paper page

Ensure the Zoro desktop app is running so the connector server is available.

## Publishing to Chrome Web Store

1. Build a production version of the extension
2. Zip the `dist/` directory:
   ```bash
   cd apps/browser-extension/dist
   zip -r ../zoro-extension.zip .
   ```
3. Go to the [Chrome Web Store Developer Dashboard](https://chrome.google.com/webstore/devconsole)
4. Upload the zip file
5. Fill in store listing details (description, screenshots, etc.)
6. Submit for review

Note: The extension requires `host_permissions` for `127.0.0.1`, which the Chrome Web Store review process treats as a local-only permission.

## Using the Official Zotero Browser Extension

Instead of (or in addition to) the Zoro browser extension, you can use the **official Zotero browser extension** to save papers to Zoro. This leverages Zotero's extensive translator library, which supports thousands of academic publisher sites.

### Setup

1. Install the [Zotero Connector](https://www.zotero.org/download/connectors) browser extension
2. In Zoro, go to **Settings > Browser Connector**
3. Enable **"Zotero Connector compatibility"**
4. Make sure Zotero desktop is **not running** (both apps cannot listen on port 23119 simultaneously)

### How It Works

When Zotero Connector compatibility is enabled, Zoro starts a second HTTP server on port **23119** (Zotero's default) that speaks the Zotero Connector protocol:

1. The Zotero browser extension detects papers using its built-in translators (runs in the browser)
2. When you click "Save to Zotero", the extension sends the parsed metadata to `localhost:23119`
3. Zoro receives the structured item data and saves it to the library
4. PDF attachments are uploaded directly by the extension and stored in the paper directory

### Supported Zotero Connector Features

| Feature | Status |
|---|---|
| Save items (papers, books, etc.) | Supported |
| Save webpage snapshots | Supported |
| PDF/EPUB attachment upload | Supported |
| SingleFile HTML snapshots | Supported |
| Collection picker (save to collection) | Supported |
| Tag assignment | Stored as labels in metadata (not sidebar tags) |
| RIS/BibTeX import | Supported |
| Session progress tracking | Supported |
| Translator sync | Not needed (translators run in browser) |
| PDF metadata recognition | Not supported |
| Open Access resolvers | Not supported |
| Google Docs integration | Not supported |

### Limitations

- **Cannot run alongside Zotero**: Both applications listen on port 23119. Close Zotero before enabling compatibility mode.
- **No PDF recognition**: Standalone PDFs saved without metadata will not be automatically recognized. Use the Zoro extension or manual entry for these cases.
- **Zotero-specific metadata**: Fields like journal volume, issue, and pages are preserved in `extra_json` but not displayed in the Zoro UI (the data is not lost).
- **Tags become labels**: Tags from the Zotero Connector are stored as read-only labels in `extra_json.labels`, not as sidebar tags. Sidebar tags in Zoro are user-curated only.

## See Also

- [Architecture Overview](architecture.md) -- Connector server details
- [Data Model & API Reference](data-model.md) -- Connector HTTP endpoints
- [Development Guide](development.md) -- Build setup
