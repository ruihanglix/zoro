// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::backend::Backend;
use crate::output;

pub fn list(backend: &dyn Backend, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let collections = backend.list_collections()?;
    output::print_collections(&collections, json);
    Ok(())
}

pub fn create(
    backend: &dyn Backend,
    name: &str,
    description: Option<&str>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let info = backend.create_collection(name, description)?;
    output::print_success(
        &format!("Created collection: {}", info.name),
        &info,
        json,
    );
    Ok(())
}

pub fn add_paper(
    backend: &dyn Backend,
    paper: &str,
    collection: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    backend.add_paper_to_collection(paper, collection)?;
    output::print_success_msg(
        &format!("Added paper {} to collection {}", paper, collection),
        json,
    );
    Ok(())
}

pub fn remove_paper(
    backend: &dyn Backend,
    paper: &str,
    collection: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    backend.remove_paper_from_collection(paper, collection)?;
    output::print_success_msg(
        &format!("Removed paper {} from collection {}", paper, collection),
        json,
    );
    Ok(())
}
