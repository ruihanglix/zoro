// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::backend::Backend;

pub fn export(
    backend: &dyn Backend,
    paper: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = backend.export_paper(paper, format)?;
    // Print raw export content (not wrapped in JSON even with --json,
    // since the content is already in the requested format)
    print!("{}", result.content);
    Ok(())
}
