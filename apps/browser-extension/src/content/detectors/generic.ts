// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { DetectionResult } from "../../lib/types";

export function detectGeneric(url: string, doc: Document): DetectionResult {
  // Look for citation meta tags (Google Scholar / Highwire Press format)
  const title = doc.querySelector('meta[name="citation_title"]')?.getAttribute("content");

  if (!title) {
    return { detected: false, source: "generic" };
  }

  const authorMetas = doc.querySelectorAll('meta[name="citation_author"]');
  const authors: string[] = [];
  authorMetas.forEach((el) => {
    const name = el.getAttribute("content");
    if (name) authors.push(name);
  });

  const doi = doc.querySelector('meta[name="citation_doi"]')?.getAttribute("content") || undefined;
  const pdf_url = doc.querySelector('meta[name="citation_pdf_url"]')?.getAttribute("content") || undefined;
  const abstract_text = doc.querySelector('meta[name="citation_abstract"]')?.getAttribute("content") || undefined;

  return {
    detected: true,
    source: "generic",
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
