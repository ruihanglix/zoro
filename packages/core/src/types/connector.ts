// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

export interface ConnectorPingResponse {
  version: string;
  name: string;
}

export interface ConnectorSaveItemRequest {
  title: string;
  authors?: string[];
  url?: string;
  doi?: string;
  arxiv_id?: string;
  pdf_url?: string;
  html_url?: string;
  abstract_text?: string;
  tags?: string[];
}

export interface ConnectorSaveItemResponse {
  success: boolean;
  paper_id?: string;
  message: string;
}
