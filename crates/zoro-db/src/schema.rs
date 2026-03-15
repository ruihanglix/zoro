// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use crate::error::DbError;
use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS papers (
            id TEXT PRIMARY KEY,
            slug TEXT UNIQUE NOT NULL,
            title TEXT NOT NULL,
            short_title TEXT,
            abstract_text TEXT,
            doi TEXT,
            arxiv_id TEXT,
            url TEXT,
            pdf_url TEXT,
            html_url TEXT,
            thumbnail_url TEXT,
            published_date TEXT,
            added_date TEXT NOT NULL,
            modified_date TEXT NOT NULL,
            source TEXT,
            read_status TEXT DEFAULT 'unread',
            rating INTEGER,
            extra_json TEXT,
            dir_path TEXT NOT NULL,
            entry_type TEXT,
            journal TEXT,
            volume TEXT,
            issue TEXT,
            pages TEXT,
            publisher TEXT,
            issn TEXT,
            isbn TEXT,
            pdf_downloaded INTEGER DEFAULT 1,
            html_downloaded INTEGER DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS authors (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            affiliation TEXT,
            orcid TEXT
        );

        CREATE TABLE IF NOT EXISTS paper_authors (
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            author_id TEXT NOT NULL REFERENCES authors(id),
            position INTEGER NOT NULL,
            PRIMARY KEY (paper_id, author_id)
        );

        CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            parent_id TEXT REFERENCES collections(id),
            position INTEGER DEFAULT 0,
            created_date TEXT NOT NULL,
            description TEXT
        );

        CREATE TABLE IF NOT EXISTS paper_collections (
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
            added_date TEXT NOT NULL,
            PRIMARY KEY (paper_id, collection_id)
        );

        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            color TEXT
        );

        CREATE TABLE IF NOT EXISTS paper_tags (
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            source TEXT DEFAULT 'manual',
            PRIMARY KEY (paper_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            content TEXT NOT NULL,
            created_date TEXT NOT NULL,
            modified_date TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS attachments (
            id TEXT PRIMARY KEY,
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            filename TEXT NOT NULL,
            file_type TEXT NOT NULL,
            mime_type TEXT,
            file_size INTEGER,
            relative_path TEXT NOT NULL,
            created_date TEXT NOT NULL,
            modified_date TEXT NOT NULL,
            source TEXT DEFAULT 'manual',
            metadata_json TEXT
        );

        CREATE TABLE IF NOT EXISTS subscriptions (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL,
            name TEXT NOT NULL,
            config_json TEXT,
            enabled INTEGER DEFAULT 1,
            poll_interval_minutes INTEGER DEFAULT 60,
            last_polled TEXT,
            created_date TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS subscription_items (
            id TEXT PRIMARY KEY,
            subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
            paper_id TEXT REFERENCES papers(id),
            external_id TEXT NOT NULL,
            title TEXT NOT NULL,
            data_json TEXT,
            fetched_date TEXT NOT NULL,
            added_to_library INTEGER DEFAULT 0,
            source_date TEXT
        );

        CREATE TABLE IF NOT EXISTS annotations (
            id TEXT PRIMARY KEY,
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            type TEXT NOT NULL,
            color TEXT NOT NULL DEFAULT '#ffe28f',
            comment TEXT,
            selected_text TEXT,
            image_data TEXT,
            position_json TEXT NOT NULL,
            page_number INTEGER NOT NULL,
            created_date TEXT NOT NULL,
            modified_date TEXT NOT NULL,
            source_file TEXT DEFAULT 'paper.pdf'
        );

        CREATE TABLE IF NOT EXISTS reader_state (
            paper_id TEXT PRIMARY KEY REFERENCES papers(id) ON DELETE CASCADE,
            scroll_position REAL,
            scale REAL DEFAULT 1.2,
            modified_date TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_changelog (
            id TEXT PRIMARY KEY,
            sequence INTEGER NOT NULL,
            device_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            field_changes_json TEXT,
            file_info_json TEXT,
            timestamp TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_state (
            device_id TEXT PRIMARY KEY,
            last_sync_time TEXT,
            last_local_sequence INTEGER NOT NULL DEFAULT 0,
            last_remote_sequences_json TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS sync_conflicts (
            id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            field TEXT,
            local_value TEXT,
            remote_value TEXT,
            remote_device_id TEXT,
            resolved INTEGER DEFAULT 0,
            created_date TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS translations (
            id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            field TEXT NOT NULL,
            target_lang TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            model TEXT,
            created_date TEXT NOT NULL,
            modified_date TEXT NOT NULL,
            UNIQUE(entity_type, entity_id, field, target_lang)
        );

        CREATE TABLE IF NOT EXISTS glossary (
            id               TEXT PRIMARY KEY,
            source_term      TEXT NOT NULL COLLATE NOCASE,
            translated_term  TEXT NOT NULL,
            target_lang      TEXT NOT NULL,
            source           TEXT NOT NULL DEFAULT 'manual',
            occurrence_count INTEGER NOT NULL DEFAULT 1,
            created_date     TEXT NOT NULL,
            updated_date     TEXT NOT NULL,
            UNIQUE(source_term COLLATE NOCASE, target_lang)
        );

        CREATE TABLE IF NOT EXISTS glossary_occurrences (
            glossary_id TEXT NOT NULL REFERENCES glossary(id) ON DELETE CASCADE,
            entity_id   TEXT NOT NULL,
            PRIMARY KEY (glossary_id, entity_id)
        );

        CREATE TABLE IF NOT EXISTS citation_cache (
            paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
            style TEXT NOT NULL,
            text TEXT NOT NULL,
            provider TEXT NOT NULL,
            doi TEXT,
            request_url TEXT,
            accept_header TEXT,
            fetched_date TEXT NOT NULL,
            PRIMARY KEY (paper_id, style)
        );

        CREATE TABLE IF NOT EXISTS plugin_storage (
            plugin_id TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (plugin_id, key)
        );

        CREATE TABLE IF NOT EXISTS papers_cool_cache (
            cache_key TEXT PRIMARY KEY,
            response_json TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            ttl_seconds INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS papers_cool_texts (
            external_id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            abstract_text TEXT
        );

        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_papers_slug ON papers(slug);
        CREATE INDEX IF NOT EXISTS idx_papers_doi ON papers(doi);
        CREATE INDEX IF NOT EXISTS idx_papers_arxiv_id ON papers(arxiv_id);
        CREATE INDEX IF NOT EXISTS idx_papers_added_date ON papers(added_date);
        CREATE INDEX IF NOT EXISTS idx_paper_authors_author ON paper_authors(author_id);
        CREATE INDEX IF NOT EXISTS idx_paper_collections_collection ON paper_collections(collection_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_collections_slug_parent
            ON collections(slug, COALESCE(parent_id, ''));
        CREATE INDEX IF NOT EXISTS idx_paper_tags_tag ON paper_tags(tag_id);
        CREATE INDEX IF NOT EXISTS idx_attachments_paper ON attachments(paper_id);
        CREATE INDEX IF NOT EXISTS idx_notes_paper ON notes(paper_id);
        CREATE INDEX IF NOT EXISTS idx_subscription_items_sub ON subscription_items(subscription_id);
        CREATE INDEX IF NOT EXISTS idx_subscription_items_ext ON subscription_items(external_id);
        CREATE INDEX IF NOT EXISTS idx_subscription_items_source_date
            ON subscription_items(subscription_id, source_date);
        CREATE INDEX IF NOT EXISTS idx_annotations_paper ON annotations(paper_id);
        CREATE INDEX IF NOT EXISTS idx_annotations_paper_page ON annotations(paper_id, page_number);
        CREATE INDEX IF NOT EXISTS idx_sync_changelog_device_seq
            ON sync_changelog(device_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_sync_changelog_timestamp
            ON sync_changelog(timestamp);
        CREATE INDEX IF NOT EXISTS idx_sync_conflicts_entity
            ON sync_conflicts(entity_type, entity_id);
        CREATE INDEX IF NOT EXISTS idx_sync_conflicts_unresolved
            ON sync_conflicts(resolved) WHERE resolved = 0;
        CREATE INDEX IF NOT EXISTS idx_translations_entity
            ON translations(entity_type, entity_id, target_lang);
        CREATE INDEX IF NOT EXISTS idx_glossary_lang ON glossary(target_lang);
        CREATE INDEX IF NOT EXISTS idx_glossary_source ON glossary(source);
        "
    )?;

    // FTS5 tables (separate because CREATE VIRTUAL TABLE IF NOT EXISTS works differently)
    let papers_fts_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='papers_fts'",
        [],
        |row| row.get(0),
    )?;

    if !papers_fts_exists {
        conn.execute_batch(
            "
            CREATE VIRTUAL TABLE papers_fts USING fts5(
                title,
                abstract_text,
                content='papers',
                content_rowid='rowid',
                tokenize='porter unicode61'
            );

            CREATE TRIGGER papers_ai AFTER INSERT ON papers BEGIN
                INSERT INTO papers_fts(rowid, title, abstract_text)
                VALUES (new.rowid, new.title, new.abstract_text);
            END;

            CREATE TRIGGER papers_ad AFTER DELETE ON papers BEGIN
                INSERT INTO papers_fts(papers_fts, rowid, title, abstract_text)
                VALUES ('delete', old.rowid, old.title, old.abstract_text);
            END;

            CREATE TRIGGER papers_au AFTER UPDATE ON papers BEGIN
                INSERT INTO papers_fts(papers_fts, rowid, title, abstract_text)
                VALUES ('delete', old.rowid, old.title, old.abstract_text);
                INSERT INTO papers_fts(rowid, title, abstract_text)
                VALUES (new.rowid, new.title, new.abstract_text);
            END;
            ",
        )?;
    }

    let translations_fts_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='translations_fts'",
        [],
        |row| row.get(0),
    )?;

    if !translations_fts_exists {
        conn.execute_batch(
            "
            CREATE VIRTUAL TABLE translations_fts USING fts5(
                translated_text,
                content='translations',
                content_rowid='rowid',
                tokenize='porter unicode61'
            );

            CREATE TRIGGER translations_ai AFTER INSERT ON translations BEGIN
                INSERT INTO translations_fts(rowid, translated_text)
                VALUES (new.rowid, new.translated_text);
            END;

            CREATE TRIGGER translations_ad AFTER DELETE ON translations BEGIN
                INSERT INTO translations_fts(translations_fts, rowid, translated_text)
                VALUES ('delete', old.rowid, old.translated_text);
            END;

            CREATE TRIGGER translations_au AFTER UPDATE ON translations BEGIN
                INSERT INTO translations_fts(translations_fts, rowid, translated_text)
                VALUES ('delete', old.rowid, old.translated_text);
                INSERT INTO translations_fts(rowid, translated_text)
                VALUES (new.rowid, new.translated_text);
            END;
            ",
        )?;
    }

    Ok(())
}
