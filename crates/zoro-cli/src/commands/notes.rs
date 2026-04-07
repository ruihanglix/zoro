// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::backend::Backend;
use crate::output;

pub fn list(
    backend: &dyn Backend,
    paper: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let notes = backend.list_notes(paper)?;
    output::print_notes(&notes, json);
    Ok(())
}

pub fn add(
    backend: &dyn Backend,
    paper: &str,
    content: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let note = backend.add_note(paper, content)?;
    output::print_success(&format!("Added note {} to paper {}", note.id, paper), &note, json);
    Ok(())
}

pub fn delete(
    backend: &dyn Backend,
    note_id: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    backend.delete_note(note_id)?;
    output::print_success_msg(&format!("Deleted note {}", note_id), json);
    Ok(())
}
