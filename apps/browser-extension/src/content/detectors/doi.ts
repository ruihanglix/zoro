// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { DetectionResult } from "../../lib/types";

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

  const title =
    doc.querySelector('meta[name="citation_title"]')?.getAttribute("content") ||
    doc.querySelector('meta[name="DC.title"]')?.getAttribute("content") ||
    doc.title;

  const authorMetas = doc.querySelectorAll('meta[name="citation_author"], meta[name="DC.creator"]');
  const authors: string[] = [];
  authorMetas.forEach((el) => {
    const name = el.getAttribute("content");
    if (name) authors.push(name);
  });

  const abstract_text =
    doc.querySelector('meta[name="citation_abstract"]')?.getAttribute("content") ||
    doc.querySelector('meta[name="DC.description"]')?.getAttribute("content") ||
    undefined;

  const pdf_url =
    doc.querySelector('meta[name="citation_pdf_url"]')?.getAttribute("content") || undefined;

  return {
    detected: true,
    source: "doi",
    metadata: {
      title,
      authors,
      url,
      doi,
      pdf_url,
      abstract_text,
    },
  };
}
