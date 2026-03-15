// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

export interface Paper {
  id: string;
  slug: string;
  title: string;
  short_title: string | null;
  authors: Author[];
  abstract_text: string | null;
  doi: string | null;
  arxiv_id: string | null;
  url: string | null;
  pdf_url: string | null;
  html_url: string | null;
  published_date: string | null;
  added_date: string;
  modified_date: string;
  source: string | null;
  tags: string[];
  collections: string[];
  attachments: AttachmentInfo[];
  notes: string[];
  read_status: "unread" | "reading" | "read";
  rating: number | null;
  extra: Record<string, unknown>;
  entry_type: string | null;
  journal: string | null;
  volume: string | null;
  issue: string | null;
  pages: string | null;
  publisher: string | null;
  issn: string | null;
  isbn: string | null;
}

export interface Author {
  name: string;
  affiliation?: string;
  orcid?: string;
}

export interface AttachmentInfo {
  filename: string;
  type: string;
  created: string;
}

export interface Collection {
  id: string;
  name: string;
  slug: string;
  parent_id: string | null;
  position: number;
  created_date: string;
  description: string | null;
}

export interface Tag {
  id: string;
  name: string;
  color: string | null;
}

export interface Attachment {
  id: string;
  paper_id: string;
  filename: string;
  file_type: string;
  mime_type: string | null;
  file_size: number | null;
  relative_path: string;
  created_date: string;
  modified_date: string;
  source: string;
  metadata: Record<string, unknown> | null;
}

export interface Subscription {
  id: string;
  source_type: string;
  name: string;
  config: Record<string, unknown> | null;
  enabled: boolean;
  poll_interval_minutes: number;
  last_polled: string | null;
  created_date: string;
}

export interface SubscriptionItem {
  id: string;
  subscription_id: string;
  paper_id: string | null;
  external_id: string;
  title: string;
  authors: Author[];
  abstract_text: string | null;
  url: string | null;
  pdf_url: string | null;
  html_url: string | null;
  upvotes: number | null;
  data: Record<string, unknown> | null;
  fetched_date: string;
  added_to_library: boolean;
}

export interface PaperMetadata {
  id: string;
  slug: string;
  title: string;
  short_title: string | null;
  authors: Author[];
  abstract: string | null;
  doi: string | null;
  arxiv_id: string | null;
  url: string | null;
  pdf_url: string | null;
  html_url: string | null;
  published_date: string | null;
  added_date: string;
  source: string | null;
  tags: string[];
  collections: string[];
  attachments: AttachmentInfo[];
  notes: string[];
  read_status: "unread" | "reading" | "read";
  rating: number | null;
  extra: Record<string, unknown>;
  entry_type: string | null;
  journal: string | null;
  volume: string | null;
  issue: string | null;
  pages: string | null;
  publisher: string | null;
  issn: string | null;
  isbn: string | null;
}
