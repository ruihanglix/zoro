// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

use std::sync::Arc;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};

use crate::resources;
use crate::state::AppState;
use crate::tools::*;

#[derive(Clone)]
pub struct ZoroMcpServer {
    state: Arc<AppState>,
    tool_router: ToolRouter<ZoroMcpServer>,
}

#[tool_router]
impl ZoroMcpServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    // === Papers ===

    #[tool(name = "add_paper", description = "Add a new paper to the library")]
    fn add_paper(
        &self,
        Parameters(input): Parameters<papers::AddPaperInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_add_paper(&self.state, input)
    }

    #[tool(
        name = "get_paper",
        description = "Get detailed information about a paper by ID"
    )]
    fn get_paper(
        &self,
        Parameters(input): Parameters<papers::GetPaperInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_get_paper(&self.state, input)
    }

    #[tool(
        name = "list_papers",
        description = "List papers with optional filters (collection, tag, status, sort, pagination)"
    )]
    fn list_papers(
        &self,
        Parameters(input): Parameters<papers::ListPapersInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_list_papers(&self.state, input)
    }

    #[tool(
        name = "search_papers",
        description = "Full-text search across paper titles and abstracts"
    )]
    fn search_papers(
        &self,
        Parameters(input): Parameters<papers::SearchPapersInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_search_papers(&self.state, input)
    }

    #[tool(name = "update_paper", description = "Update paper metadata")]
    fn update_paper(
        &self,
        Parameters(input): Parameters<papers::UpdatePaperInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_update_paper(&self.state, input)
    }

    #[tool(
        name = "update_paper_status",
        description = "Set paper read status (unread/reading/read)"
    )]
    fn update_paper_status(
        &self,
        Parameters(input): Parameters<papers::UpdatePaperStatusInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_update_paper_status(&self.state, input)
    }

    #[tool(
        name = "update_paper_rating",
        description = "Set paper rating (1-5) or null to clear"
    )]
    fn update_paper_rating(
        &self,
        Parameters(input): Parameters<papers::UpdatePaperRatingInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_update_paper_rating(&self.state, input)
    }

    #[tool(name = "delete_paper", description = "Delete a paper from the library")]
    fn delete_paper(
        &self,
        Parameters(input): Parameters<papers::DeletePaperInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_delete_paper(&self.state, input)
    }

    #[tool(
        name = "enrich_paper_metadata",
        description = "Enrich paper metadata from external APIs (CrossRef, Semantic Scholar, OpenAlex)"
    )]
    async fn enrich_paper_metadata(
        &self,
        Parameters(input): Parameters<papers::EnrichPaperInput>,
    ) -> Result<CallToolResult, McpError> {
        papers::tool_enrich_paper(&self.state, input).await
    }

    // === Collections ===

    #[tool(
        name = "create_collection",
        description = "Create a new collection for organizing papers"
    )]
    fn create_collection(
        &self,
        Parameters(input): Parameters<collections::CreateCollectionInput>,
    ) -> Result<CallToolResult, McpError> {
        collections::tool_create_collection(&self.state, input)
    }

    #[tool(
        name = "list_collections",
        description = "List all collections with paper counts"
    )]
    fn list_collections(&self) -> Result<CallToolResult, McpError> {
        collections::tool_list_collections(&self.state)
    }

    #[tool(
        name = "update_collection",
        description = "Update a collection's name, parent, or description"
    )]
    fn update_collection(
        &self,
        Parameters(input): Parameters<collections::UpdateCollectionInput>,
    ) -> Result<CallToolResult, McpError> {
        collections::tool_update_collection(&self.state, input)
    }

    #[tool(name = "delete_collection", description = "Delete a collection")]
    fn delete_collection(
        &self,
        Parameters(input): Parameters<collections::DeleteCollectionInput>,
    ) -> Result<CallToolResult, McpError> {
        collections::tool_delete_collection(&self.state, input)
    }

    #[tool(
        name = "add_paper_to_collection",
        description = "Add a paper to a collection"
    )]
    fn add_paper_to_collection(
        &self,
        Parameters(input): Parameters<collections::AddPaperToCollectionInput>,
    ) -> Result<CallToolResult, McpError> {
        collections::tool_add_paper_to_collection(&self.state, input)
    }

    #[tool(
        name = "remove_paper_from_collection",
        description = "Remove a paper from a collection"
    )]
    fn remove_paper_from_collection(
        &self,
        Parameters(input): Parameters<collections::RemovePaperFromCollectionInput>,
    ) -> Result<CallToolResult, McpError> {
        collections::tool_remove_paper_from_collection(&self.state, input)
    }

    #[tool(
        name = "get_collections_for_paper",
        description = "Get all collections containing a paper"
    )]
    fn get_collections_for_paper(
        &self,
        Parameters(input): Parameters<collections::GetCollectionsForPaperInput>,
    ) -> Result<CallToolResult, McpError> {
        collections::tool_get_collections_for_paper(&self.state, input)
    }

    // === Tags ===

    #[tool(name = "list_tags", description = "List all tags")]
    fn list_tags(&self) -> Result<CallToolResult, McpError> {
        tags::tool_list_tags(&self.state)
    }

    #[tool(name = "search_tags", description = "Search tags by name prefix")]
    fn search_tags(
        &self,
        Parameters(input): Parameters<tags::SearchTagsInput>,
    ) -> Result<CallToolResult, McpError> {
        tags::tool_search_tags(&self.state, input)
    }

    #[tool(name = "add_tag_to_paper", description = "Add a tag to a paper")]
    fn add_tag_to_paper(
        &self,
        Parameters(input): Parameters<tags::AddTagToPaperInput>,
    ) -> Result<CallToolResult, McpError> {
        tags::tool_add_tag_to_paper(&self.state, input)
    }

    #[tool(
        name = "remove_tag_from_paper",
        description = "Remove a tag from a paper"
    )]
    fn remove_tag_from_paper(
        &self,
        Parameters(input): Parameters<tags::RemoveTagFromPaperInput>,
    ) -> Result<CallToolResult, McpError> {
        tags::tool_remove_tag_from_paper(&self.state, input)
    }

    #[tool(name = "update_tag", description = "Update a tag's name or color")]
    fn update_tag(
        &self,
        Parameters(input): Parameters<tags::UpdateTagInput>,
    ) -> Result<CallToolResult, McpError> {
        tags::tool_update_tag(&self.state, input)
    }

    #[tool(name = "delete_tag", description = "Delete a tag")]
    fn delete_tag(
        &self,
        Parameters(input): Parameters<tags::DeleteTagInput>,
    ) -> Result<CallToolResult, McpError> {
        tags::tool_delete_tag(&self.state, input)
    }

    // === Notes ===

    #[tool(name = "add_note", description = "Add a note to a paper")]
    fn add_note(
        &self,
        Parameters(input): Parameters<notes::AddNoteInput>,
    ) -> Result<CallToolResult, McpError> {
        notes::tool_add_note(&self.state, input)
    }

    #[tool(name = "list_notes", description = "List all notes for a paper")]
    fn list_notes(
        &self,
        Parameters(input): Parameters<notes::ListNotesInput>,
    ) -> Result<CallToolResult, McpError> {
        notes::tool_list_notes(&self.state, input)
    }

    #[tool(name = "update_note", description = "Update a note's content")]
    fn update_note(
        &self,
        Parameters(input): Parameters<notes::UpdateNoteInput>,
    ) -> Result<CallToolResult, McpError> {
        notes::tool_update_note(&self.state, input)
    }

    #[tool(name = "delete_note", description = "Delete a note")]
    fn delete_note(
        &self,
        Parameters(input): Parameters<notes::DeleteNoteInput>,
    ) -> Result<CallToolResult, McpError> {
        notes::tool_delete_note(&self.state, input)
    }

    // === Annotations ===

    #[tool(name = "add_annotation", description = "Add a PDF annotation")]
    fn add_annotation(
        &self,
        Parameters(input): Parameters<annotations::AddAnnotationInput>,
    ) -> Result<CallToolResult, McpError> {
        annotations::tool_add_annotation(&self.state, input)
    }

    #[tool(
        name = "list_annotations",
        description = "List all annotations for a paper"
    )]
    fn list_annotations(
        &self,
        Parameters(input): Parameters<annotations::ListAnnotationsInput>,
    ) -> Result<CallToolResult, McpError> {
        annotations::tool_list_annotations(&self.state, input)
    }

    #[tool(name = "update_annotation", description = "Update an annotation")]
    fn update_annotation(
        &self,
        Parameters(input): Parameters<annotations::UpdateAnnotationInput>,
    ) -> Result<CallToolResult, McpError> {
        annotations::tool_update_annotation(&self.state, input)
    }

    #[tool(name = "delete_annotation", description = "Delete an annotation")]
    fn delete_annotation(
        &self,
        Parameters(input): Parameters<annotations::DeleteAnnotationInput>,
    ) -> Result<CallToolResult, McpError> {
        annotations::tool_delete_annotation(&self.state, input)
    }

    // === Import/Export ===

    #[tool(
        name = "import_bibtex",
        description = "Import papers from BibTeX content"
    )]
    fn import_bibtex(
        &self,
        Parameters(input): Parameters<import_export::ImportBibtexInput>,
    ) -> Result<CallToolResult, McpError> {
        import_export::tool_import_bibtex(&self.state, input)
    }

    #[tool(
        name = "export_bibtex",
        description = "Export papers as BibTeX (all or by paper IDs)"
    )]
    fn export_bibtex(
        &self,
        Parameters(input): Parameters<import_export::ExportBibtexInput>,
    ) -> Result<CallToolResult, McpError> {
        import_export::tool_export_bibtex(&self.state, input)
    }

    #[tool(name = "import_ris", description = "Import papers from RIS content")]
    fn import_ris(
        &self,
        Parameters(input): Parameters<import_export::ImportRisInput>,
    ) -> Result<CallToolResult, McpError> {
        import_export::tool_import_ris(&self.state, input)
    }

    #[tool(
        name = "export_ris",
        description = "Export papers as RIS (all or by paper IDs)"
    )]
    fn export_ris(
        &self,
        Parameters(input): Parameters<import_export::ExportRisInput>,
    ) -> Result<CallToolResult, McpError> {
        import_export::tool_export_ris(&self.state, input)
    }

    // === Citations ===

    #[tool(
        name = "get_formatted_citation",
        description = "Get a formatted citation for a paper (apa, ieee, mla, chicago, bibtex, ris)"
    )]
    async fn get_formatted_citation(
        &self,
        Parameters(input): Parameters<citations::GetFormattedCitationInput>,
    ) -> Result<CallToolResult, McpError> {
        citations::tool_get_formatted_citation(&self.state, input).await
    }

    #[tool(
        name = "get_paper_bibtex",
        description = "Get BibTeX entry for a paper"
    )]
    async fn get_paper_bibtex(
        &self,
        Parameters(input): Parameters<citations::GetPaperBibtexInput>,
    ) -> Result<CallToolResult, McpError> {
        citations::tool_get_paper_bibtex(&self.state, input).await
    }

    // === Subscriptions ===

    #[tool(
        name = "list_subscriptions",
        description = "List all feed subscriptions"
    )]
    fn list_subscriptions(&self) -> Result<CallToolResult, McpError> {
        subscriptions::tool_list_subscriptions(&self.state)
    }

    #[tool(
        name = "list_feed_items",
        description = "List feed items from a subscription with pagination"
    )]
    fn list_feed_items(
        &self,
        Parameters(input): Parameters<subscriptions::ListFeedItemsInput>,
    ) -> Result<CallToolResult, McpError> {
        subscriptions::tool_list_feed_items(&self.state, input)
    }

    #[tool(
        name = "add_feed_item_to_library",
        description = "Add a feed item to the library as a paper"
    )]
    fn add_feed_item_to_library(
        &self,
        Parameters(input): Parameters<subscriptions::AddFeedItemToLibraryInput>,
    ) -> Result<CallToolResult, McpError> {
        subscriptions::tool_add_feed_item_to_library(&self.state, input)
    }

    #[tool(
        name = "refresh_subscription",
        description = "Refresh a subscription feed to fetch new items"
    )]
    async fn refresh_subscription(
        &self,
        Parameters(input): Parameters<subscriptions::RefreshSubscriptionInput>,
    ) -> Result<CallToolResult, McpError> {
        subscriptions::tool_refresh_subscription(&self.state, input).await
    }

    // === Translations ===

    #[tool(
        name = "get_translations",
        description = "Get cached translations for a paper or feed item. Returns translated title and abstract if available."
    )]
    fn get_translations(
        &self,
        Parameters(input): Parameters<translations::GetTranslationsInput>,
    ) -> Result<CallToolResult, McpError> {
        translations::tool_get_translations(&self.state, input)
    }

    #[tool(
        name = "translate_paper",
        description = "Translate a paper's title and abstract using the configured LLM. Results are cached for future retrieval."
    )]
    async fn translate_paper(
        &self,
        Parameters(input): Parameters<translations::TranslatePaperInput>,
    ) -> Result<CallToolResult, McpError> {
        translations::tool_translate_paper(&self.state, input).await
    }

    #[tool(
        name = "translate_feed_item",
        description = "Translate a subscription feed item's title and abstract using the configured LLM. Results are cached for future retrieval."
    )]
    async fn translate_feed_item(
        &self,
        Parameters(input): Parameters<translations::TranslateFeedItemInput>,
    ) -> Result<CallToolResult, McpError> {
        translations::tool_translate_feed_item(&self.state, input).await
    }

    #[tool(
        name = "delete_translations",
        description = "Delete all cached translations for a paper or feed item"
    )]
    fn delete_translations(
        &self,
        Parameters(input): Parameters<translations::DeleteTranslationsInput>,
    ) -> Result<CallToolResult, McpError> {
        translations::tool_delete_translations(&self.state, input)
    }

    #[tool(
        name = "search_translated_text",
        description = "Full-text search across translated content (titles and abstracts). Returns matching entity IDs."
    )]
    fn search_translated_text(
        &self,
        Parameters(input): Parameters<translations::SearchTranslatedTextInput>,
    ) -> Result<CallToolResult, McpError> {
        translations::tool_search_translated_text(&self.state, input)
    }
}

#[tool_handler]
impl ServerHandler for ZoroMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("zoro-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Zoro MCP Server — AI-native literature management. \
             Manage your academic paper library: add/search/organize papers, \
             manage collections and tags, take notes, handle annotations, \
             import/export BibTeX/RIS, format citations, browse subscription feeds, \
             and translate paper titles/abstracts to your native language via LLM."
                .to_string(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        resources::list_resources(&self.state)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        resources::read_resource(&self.state, &request.uri)
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![RawResourceTemplate::new(
                "zoro://paper/{paper_id}",
                "Paper Metadata",
            )
            .with_description("Get full metadata for a specific paper by ID")
            .with_mime_type("application/json")
            .no_annotation()],
            next_cursor: None,
            meta: None,
        })
    }
}
