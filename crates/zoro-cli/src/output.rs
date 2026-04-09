// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use colored::Colorize;
use comfy_table::{presets, Table};
use serde::Serialize;

use crate::backend::{CollectionInfo, NoteInfo, PaperInfo, StatusInfo, TagInfo};

/// Print a list of papers as a table or JSON.
pub fn print_papers(papers: &[PaperInfo], json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(papers).unwrap());
        return;
    }

    if papers.is_empty() {
        println!("{}", "No papers found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_header(vec!["Slug", "Title", "Authors", "Year", "Status"]);

    for p in papers {
        let year = p
            .published_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .unwrap_or("-");
        let authors = truncate(&p.authors_display, 30);
        let title = truncate(&p.title, 50);
        let status = format_status(&p.read_status);
        table.add_row(vec![&p.slug, &title, &authors, year, &status]);
    }

    println!("{table}");
    println!("\n{}", format!("Found {} paper(s)", papers.len()).dimmed());
}

/// Print a single paper detail.
pub fn print_paper_detail(paper: &PaperInfo, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(paper).unwrap());
        return;
    }

    println!("{}", paper.title.bold());
    println!("{}", "─".repeat(60).dimmed());

    if !paper.authors_display.is_empty() {
        println!("  {} {}", "Authors:".dimmed(), paper.authors_display);
    }
    if let Some(ref date) = paper.published_date {
        println!(
            "  {}    {}",
            "Year:".dimmed(),
            date.get(..4).unwrap_or(date)
        );
    }
    println!(
        "  {}  {}",
        "Status:".dimmed(),
        format_status(&paper.read_status)
    );
    println!("  {}    {}", "Slug:".dimmed(), paper.slug);
    println!("  {}      {}", "ID:".dimmed(), paper.id);

    if let Some(ref doi) = paper.doi {
        println!("  {}     {}", "DOI:".dimmed(), doi);
    }
    if let Some(ref arxiv) = paper.arxiv_id {
        println!("  {}   {}", "arXiv:".dimmed(), arxiv);
    }
    if let Some(ref journal) = paper.journal {
        println!("  {} {}", "Journal:".dimmed(), journal);
    }
    if let Some(ref abs) = paper.abstract_text {
        println!("\n  {}", "Abstract:".dimmed());
        // Word-wrap abstract at ~80 chars
        for line in textwrap(abs, 76) {
            println!("    {}", line);
        }
    }

    if !paper.tags.is_empty() {
        println!("\n  {} {}", "Tags:".dimmed(), paper.tags.join(", "));
    }
    if !paper.collections.is_empty() {
        println!(
            "  {} {}",
            "Collections:".dimmed(),
            paper.collections.join(", ")
        );
    }
}

/// Print a list of collections.
pub fn print_collections(collections: &[CollectionInfo], json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(collections).unwrap());
        return;
    }

    if collections.is_empty() {
        println!("{}", "No collections found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_header(vec!["Name", "ID", "Paper Count", "Description"]);

    for c in collections {
        let desc = c.description.as_deref().unwrap_or("-");
        table.add_row(vec![
            &c.name,
            &c.id,
            &c.paper_count.to_string(),
            &truncate(desc, 40),
        ]);
    }

    println!("{table}");
}

/// Print a list of tags.
pub fn print_tags(tags: &[TagInfo], json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(tags).unwrap());
        return;
    }

    if tags.is_empty() {
        println!("{}", "No tags found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_header(vec!["Name", "ID", "Paper Count", "Color"]);

    for t in tags {
        let color = t.color.as_deref().unwrap_or("-");
        table.add_row(vec![&t.name, &t.id, &t.paper_count.to_string(), color]);
    }

    println!("{table}");
}

/// Print a list of notes.
pub fn print_notes(notes: &[NoteInfo], json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(notes).unwrap());
        return;
    }

    if notes.is_empty() {
        println!("{}", "No notes found.".dimmed());
        return;
    }

    for (i, note) in notes.iter().enumerate() {
        if i > 0 {
            println!("{}", "─".repeat(60).dimmed());
        }
        println!(
            "{} {} ({})",
            "Note".bold(),
            note.id.dimmed(),
            note.created_date.dimmed()
        );
        println!("{}", note.content);
    }
}

/// Print status info.
pub fn print_status(status: &StatusInfo, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(status).unwrap());
        return;
    }

    println!("{}", "Zoro Library Status".bold());
    println!("{}", "─".repeat(40).dimmed());
    println!("  {} {}", "Mode:".dimmed(), status.mode.green());
    println!("  {} {}", "Data dir:".dimmed(), status.data_dir);
    println!("  {} {}", "Papers:".dimmed(), status.paper_count);
    println!("  {} {}", "Collections:".dimmed(), status.collection_count);
    println!("  {} {}", "Tags:".dimmed(), status.tag_count);
}

/// Print a generic success message or JSON.
pub fn print_success<T: Serialize>(message: &str, data: &T, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(data).unwrap());
    } else {
        println!("{} {}", "✓".green(), message);
    }
}

/// Print a generic success message (no data).
pub fn print_success_msg(message: &str, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"ok": true, "message": message}))
                .unwrap()
        );
    } else {
        println!("{} {}", "✓".green(), message);
    }
}

fn format_status(status: &str) -> String {
    match status {
        "read" => "Read".green().to_string(),
        "reading" => "Reading".yellow().to_string(),
        "unread" => "Unread".dimmed().to_string(),
        other => other.to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

/// Simple word-wrap for terminal output.
fn textwrap(s: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in s.split_whitespace() {
        if current.len() + word.len() + 1 > width && !current.is_empty() {
            lines.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
