// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::backend::Backend;
use crate::output;

pub fn list(backend: &dyn Backend, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let tags = backend.list_tags()?;
    output::print_tags(&tags, json);
    Ok(())
}

pub fn add(
    backend: &dyn Backend,
    paper: &str,
    tag: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    backend.add_tag_to_paper(paper, tag)?;
    output::print_success_msg(&format!("Added tag '{}' to paper {}", tag, paper), json);
    Ok(())
}

pub fn remove(
    backend: &dyn Backend,
    paper: &str,
    tag: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    backend.remove_tag_from_paper(paper, tag)?;
    output::print_success_msg(&format!("Removed tag '{}' from paper {}", tag, paper), json);
    Ok(())
}
