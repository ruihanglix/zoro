// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

pub mod crossref;
pub mod dblp;
pub mod doi_content_negotiation;
pub mod error;
pub mod openalex;
pub mod pdf_extract;
pub mod pdf_resolve;
pub mod semantic_scholar;
pub mod unpaywall;

pub use error::MetadataError;
use serde::{Deserialize, Serialize};

/// Result of enriching a paper's metadata from external APIs.
/// Fields are `Some` only when the API returned data.
#[derive(Debug, Clone, Default)]
pub struct EnrichmentResult {
    pub entry_type: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub isbn: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub pdf_url: Option<String>,
    pub published_date: Option<String>,
    pub abstract_text: Option<String>,
    pub authors: Option<Vec<(String, Option<String>)>>, // (name, affiliation)
    pub fields_of_study: Option<Vec<String>>,           // e.g. ["Computer Science", "Mathematics"]
}

/// Main enrichment pipeline.
///
/// 1. Has DOI? → CrossRef (primary metadata source)
/// 2. Has arXiv ID? → Semantic Scholar (to discover DOI) → then CrossRef
/// 3. No identifiers but has title? → Semantic Scholar title search to discover DOI/arXiv
/// 4. Merge results (CrossRef preferred, Semantic Scholar fills gaps)
/// 5. Resolve OA PDF URL if not already known
pub async fn enrich_paper(
    doi: Option<&str>,
    arxiv_id: Option<&str>,
) -> Result<EnrichmentResult, MetadataError> {
    enrich_paper_with_title(doi, arxiv_id, None).await
}

/// Like `enrich_paper`, but also accepts a title for title-based search
/// when no DOI or arXiv ID is available.
pub async fn enrich_paper_with_title(
    doi: Option<&str>,
    arxiv_id: Option<&str>,
    title: Option<&str>,
) -> Result<EnrichmentResult, MetadataError> {
    let mut result = EnrichmentResult::default();
    let mut resolved_doi: Option<String> = doi.map(String::from);
    let mut resolved_arxiv: Option<String> = arxiv_id.map(String::from);

    // If arXiv paper, derive PDF URL immediately (no API call)
    if let Some(ref arxiv) = resolved_arxiv {
        let arxiv = arxiv.trim();
        if !arxiv.is_empty() {
            result.pdf_url = Some(format!("https://arxiv.org/pdf/{}", arxiv));
        }
    }

    // If we have neither DOI nor arXiv but have a title, search S2 by title
    // and fallback to OpenAlex if S2 fails (e.g. 429 rate-limit).
    if resolved_doi.is_none() && resolved_arxiv.is_none() {
        if let Some(t) = title {
            let t = t.trim();
            if !t.is_empty() {
                // Title search fallback chain:
                // 1. OpenAlex (free, generous rate limits)
                // 2. DBLP (free, no API key)
                // 3. Semantic Scholar (often hits 429 rate limits)
                // 4. CrossRef bibliographic search (slowest)

                // --- 1. OpenAlex title search ---
                match openalex::search_by_title(t).await {
                    Ok(Some(oa)) => {
                        if let Some(doi_str) = oa.extracted_doi() {
                            if let Some(arxiv) = extract_arxiv_from_doi(&doi_str) {
                                resolved_arxiv = Some(arxiv.clone());
                                if result.pdf_url.is_none() {
                                    result.pdf_url =
                                        Some(format!("https://arxiv.org/pdf/{}", arxiv));
                                }
                            }
                            resolved_doi = Some(doi_str.clone());
                            result.doi = Some(doi_str);
                        }
                        merge_openalex(&mut result, &oa);
                        tracing::debug!("OpenAlex title search found match for: {}", t);
                    }
                    Ok(None) => {
                        tracing::debug!("No OpenAlex title match for: {}", t);
                    }
                    Err(e) => {
                        tracing::debug!("OpenAlex title search failed for \"{}\": {}", t, e);
                    }
                }

                // --- 2. DBLP title search ---
                if resolved_doi.is_none() && resolved_arxiv.is_none() {
                    match dblp::search_by_query(t, 1).await {
                        Ok(hits) => {
                            if let Some(dh) = hits.into_iter().next() {
                                if let Some(ref doi_str) = dh.doi {
                                    if let Some(arxiv) = extract_arxiv_from_doi(doi_str) {
                                        resolved_arxiv = Some(arxiv.clone());
                                        if result.pdf_url.is_none() {
                                            result.pdf_url =
                                                Some(format!("https://arxiv.org/pdf/{}", arxiv));
                                        }
                                    }
                                    resolved_doi = Some(doi_str.clone());
                                    result.doi = Some(doi_str.clone());
                                }
                                // Merge DBLP metadata (authors, venue, year, etc.)
                                if result.published_date.is_none() {
                                    if let Some(ref y) = dh.year {
                                        result.published_date = Some(format!("{}-01-01", y));
                                    }
                                }
                                if result.journal.is_none() {
                                    result.journal = dh.venue.clone();
                                }
                                if result.authors.is_none() && !dh.authors.is_empty() {
                                    result.authors = Some(
                                        dh.authors
                                            .iter()
                                            .map(|name| (name.clone(), None))
                                            .collect(),
                                    );
                                }
                                tracing::debug!("DBLP title search found match for: {}", t);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("DBLP title search failed for \"{}\": {}", t, e);
                        }
                    }
                }

                // --- 3. Semantic Scholar title search ---
                if resolved_doi.is_none() && resolved_arxiv.is_none() {
                    match semantic_scholar::search_by_title(t).await {
                        Ok(Some(s2)) => {
                            if let Some(ref ext) = s2.external_ids {
                                if let Some(d) = ext.get("DOI").and_then(|v| v.as_str()) {
                                    resolved_doi = Some(d.to_string());
                                    result.doi = Some(d.to_string());
                                }
                                if let Some(a) = ext.get("ArXiv").and_then(|v| v.as_str()) {
                                    resolved_arxiv = Some(a.to_string());
                                    if result.pdf_url.is_none() {
                                        result.pdf_url =
                                            Some(format!("https://arxiv.org/pdf/{}", a));
                                    }
                                }
                            }
                            merge_semantic_scholar(&mut result, &s2);
                            tracing::debug!("S2 title search found match for: {}", t);
                        }
                        Ok(None) => {
                            tracing::debug!("No S2 title match for: {}", t);
                        }
                        Err(e) => {
                            tracing::debug!("S2 title search failed for \"{}\": {}", t, e);
                        }
                    }
                }

                // --- 4. CrossRef bibliographic search ---
                if resolved_doi.is_none() && resolved_arxiv.is_none() {
                    let client = reqwest::Client::new();
                    let resp = client
                        .get("https://api.crossref.org/works")
                        .query(&[
                            ("query.bibliographic", t),
                            ("rows", "1"),
                            ("sort", "relevance"),
                            ("order", "desc"),
                        ])
                        .header(
                            "User-Agent",
                            "Zoro/0.1 (https://github.com/ruihanglix/zoro; mailto:zoro@gmail.com)",
                        )
                        .send()
                        .await;

                    match resp {
                        Ok(r) if r.status().is_success() => {
                            #[derive(Deserialize)]
                            struct Resp {
                                message: Option<CrMsg>,
                            }
                            #[derive(Deserialize)]
                            struct CrMsg {
                                items: Option<Vec<crossref::CrossRefWork>>,
                            }
                            if let Some(cr) = r
                                .json::<Resp>()
                                .await
                                .ok()
                                .and_then(|b| b.message)
                                .and_then(|m| m.items)
                                .and_then(|v| v.into_iter().next())
                            {
                                if let Some(ref doi_str) = cr.doi {
                                    if let Some(arxiv) = extract_arxiv_from_doi(doi_str) {
                                        resolved_arxiv = Some(arxiv.clone());
                                        if result.pdf_url.is_none() {
                                            result.pdf_url =
                                                Some(format!("https://arxiv.org/pdf/{}", arxiv));
                                        }
                                    }
                                    resolved_doi = Some(doi_str.clone());
                                    result.doi = Some(doi_str.clone());
                                    merge_crossref(&mut result, &cr);
                                    tracing::debug!(
                                        "CrossRef bibliographic search found match for: {}",
                                        t
                                    );
                                }
                            }
                        }
                        _ => {
                            tracing::debug!("CrossRef bibliographic search failed for: {}", t);
                        }
                    }
                }
            }
        }
    }

    // If we only have arXiv ID, try Semantic Scholar to find the DOI
    if resolved_doi.is_none() {
        if let Some(ref arxiv) = resolved_arxiv {
            let s2_id = format!("ArXiv:{}", arxiv);
            match semantic_scholar::fetch_semantic_scholar(&s2_id).await {
                Ok(s2) => {
                    if let Some(ref ext) = s2.external_ids {
                        if let Some(d) = ext.get("DOI").and_then(|v| v.as_str()) {
                            resolved_doi = Some(d.to_string());
                            result.doi = Some(d.to_string());
                        }
                    }
                    merge_semantic_scholar(&mut result, &s2);
                }
                Err(e) => {
                    tracing::debug!("Semantic Scholar lookup failed for {}: {}", arxiv, e);
                }
            }
        }
    } else if let Some(ref doi_str) = resolved_doi {
        // We have a DOI — also query S2 for OA PDF and publication types
        let s2_id = format!("DOI:{}", doi_str);
        match semantic_scholar::fetch_semantic_scholar(&s2_id).await {
            Ok(s2) => {
                merge_semantic_scholar(&mut result, &s2);
            }
            Err(e) => {
                tracing::debug!("Semantic Scholar lookup (DOI) failed: {}", e);
            }
        }
    }

    // If we have a DOI (original or discovered), use CrossRef
    if let Some(ref doi_str) = resolved_doi {
        match crossref::fetch_crossref_metadata(doi_str).await {
            Ok(cr) => {
                merge_crossref(&mut result, &cr);
            }
            Err(e) => {
                tracing::debug!("CrossRef lookup failed for {}: {}", doi_str, e);
                // Fallback: try OpenAlex
                match openalex::fetch_openalex(doi_str).await {
                    Ok(oa) => {
                        merge_openalex(&mut result, &oa);
                    }
                    Err(e2) => {
                        tracing::debug!("OpenAlex lookup failed for {}: {}", doi_str, e2);
                    }
                }
            }
        }
    }

    // If we still don't have a PDF URL, try Unpaywall / OpenAlex OA endpoints
    if result.pdf_url.is_none() {
        if let Some(ref doi_str) = resolved_doi {
            match unpaywall::fetch_unpaywall(doi_str).await {
                Ok(resp) => {
                    if let Some(url) = resp.pdf_url() {
                        result.pdf_url = Some(url.to_string());
                    }
                }
                Err(e) => {
                    tracing::debug!("Unpaywall lookup failed: {}", e);
                }
            }
        }
    }

    if result.pdf_url.is_none() {
        if let Some(ref doi_str) = resolved_doi {
            match openalex::fetch_openalex(doi_str).await {
                Ok(oa) => {
                    if let Some(url) = oa.oa_pdf_url() {
                        result.pdf_url = Some(url.to_string());
                    }
                }
                Err(e) => {
                    tracing::debug!("OpenAlex OA PDF lookup failed: {}", e);
                }
            }
        }
    }

    // Propagate discovered arXiv ID (may have been found via title search or S2)
    if result.arxiv_id.is_none() {
        result.arxiv_id = resolved_arxiv;
    }

    Ok(result)
}

fn merge_crossref(result: &mut EnrichmentResult, cr: &crossref::CrossRefWork) {
    if result.entry_type.is_none() {
        result.entry_type = cr.work_type.as_ref().map(|t| crossref_type_to_entry(t));
    }
    if result.journal.is_none() {
        result.journal = cr.container_title.as_ref().and_then(|v| v.first()).cloned();
    }
    if result.volume.is_none() {
        result.volume = cr.volume.clone();
    }
    if result.issue.is_none() {
        result.issue = cr.issue.clone();
    }
    if result.pages.is_none() {
        result.pages = cr.page.clone();
    }
    if result.publisher.is_none() {
        result.publisher = cr.publisher.clone();
    }
    if result.issn.is_none() {
        result.issn = cr.issn.as_ref().and_then(|v| v.first()).cloned();
    }
    if result.isbn.is_none() {
        result.isbn = cr.isbn.as_ref().and_then(|v| v.first()).cloned();
    }
    if result.published_date.is_none() {
        result.published_date = cr.published_date();
    }
    if result.abstract_text.is_none() {
        result.abstract_text = cr.abstract_text.clone();
    }
}

fn merge_semantic_scholar(result: &mut EnrichmentResult, s2: &semantic_scholar::S2Paper) {
    if result.journal.is_none() {
        if let Some(ref j) = s2.journal {
            result.journal = j.name.clone();
            if result.volume.is_none() {
                result.volume = j.volume.clone();
            }
            if result.pages.is_none() {
                result.pages = j.pages.clone();
            }
        }
    }
    if result.published_date.is_none() {
        result.published_date = s2.publication_date.clone();
    }
    if result.abstract_text.is_none() {
        result.abstract_text = s2.abstract_text.clone();
    }
    // Derive entry_type from S2 publication types
    if result.entry_type.is_none() {
        if let Some(ref types) = s2.publication_types {
            result.entry_type = s2_types_to_entry(types);
        }
    }
    // OA PDF URL from S2
    if result.pdf_url.is_none() {
        if let Some(ref oa) = s2.open_access_pdf {
            if let Some(ref url) = oa.url {
                result.pdf_url = Some(url.clone());
            }
        }
    }
    // Extract fields of study (prefer s2-generated categories)
    if result.fields_of_study.is_none() {
        if let Some(ref fields) = s2.s2_fields_of_study {
            let categories: Vec<String> =
                fields.iter().filter_map(|f| f.category.clone()).collect();
            if !categories.is_empty() {
                result.fields_of_study = Some(categories);
            }
        }
    }
}

fn s2_types_to_entry(types: &[String]) -> Option<String> {
    for t in types {
        match t.as_str() {
            "JournalArticle" => return Some("article".to_string()),
            "Conference" => return Some("inproceedings".to_string()),
            "Book" => return Some("book".to_string()),
            "BookSection" => return Some("incollection".to_string()),
            "Review" => return Some("article".to_string()),
            "Dataset" => return Some("misc".to_string()),
            _ => {}
        }
    }
    None
}

fn merge_openalex(result: &mut EnrichmentResult, oa: &openalex::OpenAlexWork) {
    if result.entry_type.is_none() {
        result.entry_type = oa.work_type.as_ref().map(|t| openalex_type_to_entry(t));
    }
    if result.journal.is_none() {
        if let Some(ref src) = oa.primary_location {
            if let Some(ref source) = src.source {
                result.journal = source.display_name.clone();
                if result.issn.is_none() {
                    result.issn = source.issn_l.clone();
                }
            }
        }
    }
    if let Some(ref biblio) = oa.biblio {
        if result.volume.is_none() {
            result.volume = biblio.volume.clone();
        }
        if result.issue.is_none() {
            result.issue = biblio.issue.clone();
        }
        if result.pages.is_none() {
            match (&biblio.first_page, &biblio.last_page) {
                (Some(fp), Some(lp)) => result.pages = Some(format!("{}-{}", fp, lp)),
                (Some(fp), None) => result.pages = Some(fp.clone()),
                _ => {}
            }
        }
    }
    if result.published_date.is_none() {
        result.published_date = oa.publication_date.clone();
    }
}

fn crossref_type_to_entry(crossref_type: &str) -> String {
    match crossref_type {
        "journal-article" => "article".to_string(),
        "proceedings-article" => "inproceedings".to_string(),
        "book" | "monograph" => "book".to_string(),
        "book-chapter" => "incollection".to_string(),
        "dissertation" => "phdthesis".to_string(),
        "report" => "techreport".to_string(),
        "posted-content" => "misc".to_string(),
        other => other.to_string(),
    }
}

fn openalex_type_to_entry(openalex_type: &str) -> String {
    match openalex_type {
        "article" | "journal-article" => "article".to_string(),
        "proceedings-article" | "proceedings" => "inproceedings".to_string(),
        "book" => "book".to_string(),
        "book-chapter" => "incollection".to_string(),
        "dissertation" => "phdthesis".to_string(),
        "report" => "techreport".to_string(),
        other => other.to_string(),
    }
}

/// A single metadata search result returned to the UI for manual selection.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataCandidate {
    /// Which API returned this result ("Semantic Scholar" or "OpenAlex")
    pub source: String,
    pub title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub year: Option<i32>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub abstract_text: Option<String>,
}

/// Structured search parameters for metadata search.
/// Each field is optional; the search combines non-empty fields to build
/// queries for Semantic Scholar and OpenAlex APIs, similar to Zotero's
/// advanced search.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MetadataSearchParams {
    pub title: Option<String>,
    pub author: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub year: Option<String>,
    pub journal: Option<String>,
    pub isbn: Option<String>,
}

impl MetadataSearchParams {
    /// Build a free-text query string from the structured fields,
    /// suitable for Semantic Scholar's /paper/search endpoint.
    fn to_s2_query(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref t) = self.title {
            let t = t.trim();
            if !t.is_empty() {
                parts.push(t.to_string());
            }
        }
        if let Some(ref a) = self.author {
            let a = a.trim();
            if !a.is_empty() {
                parts.push(a.to_string());
            }
        }
        parts.join(" ")
    }

    /// Build OpenAlex filter string from structured fields.
    fn to_oa_filter(&self) -> Option<String> {
        let mut filters = Vec::new();
        if let Some(ref y) = self.year {
            let y = y.trim();
            if !y.is_empty() {
                filters.push(format!("publication_year:{}", y));
            }
        }
        if let Some(ref j) = self.journal {
            let j = j.trim();
            if !j.is_empty() {
                filters.push(format!("primary_location.source.display_name.search:{}", j));
            }
        }
        if let Some(ref doi) = self.doi {
            let doi = doi.trim();
            if !doi.is_empty() {
                filters.push(format!("doi:https://doi.org/{}", doi));
            }
        }
        if !filters.is_empty() {
            Some(filters.join(","))
        } else {
            None
        }
    }

    /// Build OpenAlex search query from title + author.
    fn to_oa_search(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref t) = self.title {
            let t = t.trim();
            if !t.is_empty() {
                parts.push(t.to_string());
            }
        }
        if let Some(ref a) = self.author {
            let a = a.trim();
            if !a.is_empty() {
                parts.push(a.to_string());
            }
        }
        parts.join(" ")
    }

    /// Check if there is any usable search criterion.
    fn is_empty(&self) -> bool {
        let all_blank = |s: &Option<String>| s.as_ref().is_none_or(|v| v.trim().is_empty());
        all_blank(&self.title)
            && all_blank(&self.author)
            && all_blank(&self.doi)
            && all_blank(&self.arxiv_id)
            && all_blank(&self.year)
            && all_blank(&self.journal)
            && all_blank(&self.isbn)
    }
}

/// Search multiple APIs by query (title, DOI, arXiv ID, or free text) and
/// return a list of candidate results for the user to pick from.
///
/// Unlike the automatic enrichment pipeline which picks the best match,
/// this returns *all* candidates so the user can decide.
pub async fn search_metadata_candidates(params: &MetadataSearchParams) -> Vec<MetadataCandidate> {
    if params.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();

    // If DOI or arXiv ID is given directly, do a direct lookup first
    if let Some(ref doi) = params.doi {
        let doi = doi.trim();
        if !doi.is_empty() {
            // Direct S2 lookup by DOI
            let s2_id = format!("DOI:{}", doi);
            if let Ok(p) = semantic_scholar::fetch_semantic_scholar(&s2_id).await {
                let (d, a) = extract_s2_external_ids(&p);
                candidates.push(MetadataCandidate {
                    source: "Semantic Scholar".into(),
                    title: p.title.clone(),
                    authors: p
                        .authors
                        .as_ref()
                        .map(|a| a.iter().filter_map(|au| au.name.clone()).collect()),
                    year: p.year,
                    venue: p.venue.clone().filter(|v| !v.is_empty()),
                    doi: d,
                    arxiv_id: a,
                    abstract_text: p.abstract_text.clone(),
                });
            }
        }
    }

    if let Some(ref arxiv) = params.arxiv_id {
        let arxiv = arxiv.trim();
        if !arxiv.is_empty() {
            let s2_id = format!("ArXiv:{}", arxiv);
            if let Ok(p) = semantic_scholar::fetch_semantic_scholar(&s2_id).await {
                let (d, a) = extract_s2_external_ids(&p);
                candidates.push(MetadataCandidate {
                    source: "Semantic Scholar".into(),
                    title: p.title.clone(),
                    authors: p
                        .authors
                        .as_ref()
                        .map(|a| a.iter().filter_map(|au| au.name.clone()).collect()),
                    year: p.year,
                    venue: p.venue.clone().filter(|v| !v.is_empty()),
                    doi: d,
                    arxiv_id: a,
                    abstract_text: p.abstract_text.clone(),
                });
            }
        }
    }

    // Build free-text search query from title + author fields
    let s2_query = params.to_s2_query();
    let oa_search = params.to_oa_search();
    let oa_filter = params.to_oa_filter();

    // Only do text searches if there's a query to search for, or filters to apply
    let do_s2_search = !s2_query.is_empty();
    let do_oa_search = !oa_search.is_empty() || oa_filter.is_some();

    // Search Semantic Scholar
    let s2_future = async {
        if !do_s2_search {
            return None;
        }
        let client = reqwest::Client::new();
        let mut req = client
            .get(format!(
                "https://api.semanticscholar.org/graph/v1/paper/search?fields={}&limit=5",
                "title,abstract,year,venue,externalIds,authors"
            ))
            .query(&[("query", s2_query.as_str())]);

        // S2 supports year range filter
        if let Some(ref y) = params.year {
            let y = y.trim();
            if !y.is_empty() {
                req = req.query(&[("year", y)]);
            }
        }

        let resp = req.header("User-Agent", "Zoro/0.1").send().await;

        match resp {
            Ok(r) if r.status().is_success() => {
                #[derive(Deserialize)]
                struct Resp {
                    data: Option<Vec<semantic_scholar::S2Paper>>,
                }
                r.json::<Resp>().await.ok().and_then(|b| b.data)
            }
            _ => None,
        }
    };

    // Search OpenAlex
    let oa_future = async {
        if !do_oa_search {
            return None;
        }
        let client = reqwest::Client::new();
        let mut params_list: Vec<(&str, String)> = vec![
            ("mailto", "zoro@gmail.com".to_string()),
            ("per_page", "5".to_string()),
        ];

        if !oa_search.is_empty() {
            params_list.push(("search", oa_search.clone()));
        }

        if let Some(ref f) = oa_filter {
            params_list.push(("filter", f.clone()));
        }

        let resp = client
            .get("https://api.openalex.org/works")
            .query(&params_list)
            .header("User-Agent", "Zoro/0.1")
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                #[derive(Deserialize)]
                struct Resp {
                    results: Option<Vec<openalex::OpenAlexWork>>,
                }
                r.json::<Resp>().await.ok().and_then(|b| b.results)
            }
            _ => None,
        }
    };

    // Search CrossRef by query (bibliographic search)
    let cr_query = params.to_s2_query(); // reuse title+author text
    let cr_future = async {
        if cr_query.is_empty() {
            return None;
        }
        let client = reqwest::Client::new();
        let resp = client
            .get("https://api.crossref.org/works")
            .query(&[
                ("query.bibliographic", cr_query.as_str()),
                ("rows", "5"),
                ("sort", "relevance"),
                ("order", "desc"),
            ])
            .header(
                "User-Agent",
                "Zoro/0.1 (https://github.com/ruihanglix/zoro; mailto:zoro@gmail.com)",
            )
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                #[derive(Deserialize)]
                struct Resp {
                    message: Option<CrSearchMessage>,
                }
                #[derive(Deserialize)]
                struct CrSearchMessage {
                    items: Option<Vec<crossref::CrossRefWork>>,
                }
                r.json::<Resp>()
                    .await
                    .ok()
                    .and_then(|b| b.message)
                    .and_then(|m| m.items)
            }
            _ => None,
        }
    };

    // Search DBLP
    let dblp_query = params.to_s2_query(); // reuse title+author text
    let dblp_future = async {
        if dblp_query.is_empty() {
            return Err(MetadataError::NotFound("empty query".into()));
        }
        dblp::search_by_query(&dblp_query, 5).await
    };

    // Run all four searches concurrently
    let (s2_results, oa_results, cr_results, dblp_results) =
        tokio::join!(s2_future, oa_future, cr_future, dblp_future);

    // Convert S2 results
    if let Some(papers) = s2_results {
        for p in papers {
            let (doi, arxiv) = extract_s2_external_ids(&p);
            candidates.push(MetadataCandidate {
                source: "Semantic Scholar".into(),
                title: p.title.clone(),
                authors: p
                    .authors
                    .as_ref()
                    .map(|a| a.iter().filter_map(|au| au.name.clone()).collect()),
                year: p.year,
                venue: p.venue.clone().filter(|v| !v.is_empty()),
                doi,
                arxiv_id: arxiv,
                abstract_text: p.abstract_text.clone(),
            });
        }
    }

    // Convert OpenAlex results
    if let Some(works) = oa_results {
        for w in works {
            let doi = w.extracted_doi();
            let arxiv = doi.as_ref().and_then(|d| extract_arxiv_from_doi(d));
            let venue = w
                .primary_location
                .as_ref()
                .and_then(|l| l.source.as_ref())
                .and_then(|s| s.display_name.clone());
            let year = w
                .publication_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<i32>().ok());
            candidates.push(MetadataCandidate {
                source: "OpenAlex".into(),
                title: w.title.clone(),
                authors: None, // OpenAlex basic search doesn't include authors inline
                year,
                venue,
                doi,
                arxiv_id: arxiv,
                abstract_text: None,
            });
        }
    }

    // Convert CrossRef results
    if let Some(works) = cr_results {
        for w in works {
            let doi = w.doi.clone();
            let arxiv = doi.as_ref().and_then(|d| extract_arxiv_from_doi(d));
            let title = w.title.as_ref().and_then(|v| v.first()).cloned();
            let authors = w.author.as_ref().map(|a| {
                a.iter()
                    .filter_map(|au| {
                        if let (Some(g), Some(f)) = (&au.given, &au.family) {
                            Some(format!("{} {}", g, f))
                        } else {
                            au.family.clone().or(au.name.clone())
                        }
                    })
                    .collect()
            });
            let venue = w.container_title.as_ref().and_then(|v| v.first()).cloned();
            let year = w
                .published_date()
                .and_then(|d| d.split('-').next().map(String::from))
                .and_then(|y| y.parse::<i32>().ok());
            candidates.push(MetadataCandidate {
                source: "CrossRef".into(),
                title,
                authors,
                year,
                venue,
                doi,
                arxiv_id: arxiv,
                abstract_text: w.abstract_text.clone(),
            });
        }
    }

    // Convert DBLP results
    if let Ok(hits) = dblp_results {
        for h in hits {
            let authors = if h.authors.is_empty() {
                None
            } else {
                Some(h.authors)
            };
            let year = h.year.and_then(|y| y.parse::<i32>().ok());
            let arxiv = h.doi.as_ref().and_then(|d| extract_arxiv_from_doi(d));
            candidates.push(MetadataCandidate {
                source: "DBLP".into(),
                title: h.title,
                authors,
                year,
                venue: h.venue,
                doi: h.doi,
                arxiv_id: arxiv,
                abstract_text: None,
            });
        }
    }

    candidates
}

/// Extract DOI and arXiv ID from S2 external IDs JSON value.
fn extract_s2_external_ids(paper: &semantic_scholar::S2Paper) -> (Option<String>, Option<String>) {
    let ext = match &paper.external_ids {
        Some(v) => v,
        None => return (None, None),
    };
    let doi = ext.get("DOI").and_then(|v| v.as_str()).map(String::from);
    let arxiv = ext.get("ArXiv").and_then(|v| v.as_str()).map(String::from);
    (doi, arxiv)
}

/// Extract arXiv ID from an arXiv-style DOI (e.g. "10.48550/arXiv.2509.05952").
fn extract_arxiv_from_doi(doi: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)10\.48550/arXiv\.(\d{4}\.\d{4,5}(?:v\d+)?)").unwrap());
    RE.captures(doi)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}
