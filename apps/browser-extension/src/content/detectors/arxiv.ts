// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { DetectionResult } from "../../lib/types";

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
    doc.title.replace(" - arXiv", "").trim() ||
    "";

  const authorElements = doc.querySelectorAll(".authors a, meta[name='citation_author']");
  const authors: string[] = [];
  authorElements.forEach((el) => {
    const name = el.getAttribute("content") || el.textContent?.trim();
    if (name && !authors.includes(name)) {
      authors.push(name);
    }
  });

  const abstractEl = doc.querySelector(".abstract");
  const abstract_text = abstractEl
    ? abstractEl.textContent?.replace(/^Abstract:\s*/, "").trim()
    : doc.querySelector('meta[name="citation_abstract"]')?.getAttribute("content") || undefined;

  const doi = doc.querySelector('meta[name="citation_doi"]')?.getAttribute("content") || undefined;

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
