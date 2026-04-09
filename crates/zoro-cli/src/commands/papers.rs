// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::backend::Backend;
use crate::output;

pub fn search(
    backend: &dyn Backend,
    query: &str,
    limit: i64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let papers = backend.search_papers(query, limit)?;
    output::print_papers(&papers, json);
    Ok(())
}

pub fn list(
    backend: &dyn Backend,
    collection: Option<&str>,
    tag: Option<&str>,
    status: Option<&str>,
    limit: i64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let papers = backend.list_papers(collection, tag, status, limit)?;
    output::print_papers(&papers, json);
    Ok(())
}

pub fn get(
    backend: &dyn Backend,
    paper: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let info = backend.get_paper(paper)?;
    output::print_paper_detail(&info, json);
    Ok(())
}

pub fn add(
    backend: &dyn Backend,
    source: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let info = backend.add_paper(source)?;
    output::print_success(
        &format!("Added paper: {} ({})", info.title, info.slug),
        &info,
        json,
    );
    Ok(())
}

pub fn open(backend: &dyn Backend, paper: &str) -> Result<(), Box<dyn std::error::Error>> {
    backend.open_paper(paper)?;
    Ok(())
}

pub fn delete(
    backend: &dyn Backend,
    paper: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    backend.delete_paper(paper)?;
    output::print_success_msg(&format!("Deleted paper: {}", paper), json);
    Ok(())
}
