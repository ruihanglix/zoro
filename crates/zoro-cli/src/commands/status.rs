// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::backend::Backend;
use crate::output;

pub fn status(backend: &dyn Backend, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let info = backend.status()?;
    output::print_status(&info, json);
    Ok(())
}
