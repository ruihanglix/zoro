// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use sha2::{Digest, Sha256};
use slug::slugify;

pub fn generate_paper_slug(title: &str, identifier: &str, year: Option<&str>) -> String {
    let year_str = year
        .map(|y| y[..4].to_string())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y").to_string());

    let title_slug = slugify(title);
    let truncated = truncate_on_word_boundary(&title_slug, 40);

    let mut hasher = Sha256::new();
    hasher.update(identifier.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let short_hash = &hash[..8];

    format!("{}-{}-{}", year_str, truncated, short_hash)
}

fn truncate_on_word_boundary(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let truncated = &s[..max_len];
    match truncated.rfind('-') {
        Some(pos) if pos > 10 => &truncated[..pos],
        _ => truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug() {
        let slug = generate_paper_slug(
            "Attention Is All You Need",
            "1706.03762",
            Some("2017-06-12"),
        );
        assert!(slug.starts_with("2017-attention-is-all-you-need-"));
        assert_eq!(slug.len(), "2017-attention-is-all-you-need-".len() + 8);
    }

    #[test]
    fn test_long_title_truncation() {
        let slug = generate_paper_slug(
            "A Very Long Title That Should Be Truncated On A Word Boundary To Ensure Readability",
            "test-id",
            Some("2024"),
        );
        // Verify the title part (between year- and -hash) is <= 40 chars
        let parts: Vec<&str> = slug.splitn(2, '-').collect();
        let rest = parts[1]; // everything after year-
        let hash_start = rest.len() - 8;
        let title_part = &rest[..hash_start - 1]; // exclude the trailing dash
        assert!(title_part.len() <= 40);
    }
}
