use crate::helpers::model_read::{resolve_cards, resolve_column_in_board, resolve_sprint_in_board};
use crate::helpers::{
    card_board, kanban_err_to_mcp, locked_write, to_call_tool_result, to_call_tool_result_json,
};
use crate::requests::card::{
    ArchiveCardsRequest, AssignCardToSprintRequest, AssignCardsToSprintRequest, MoveCardsRequest,
    UnassignCardFromSprintRequest,
};
use crate::scope::{Ref, ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_domain::KanbanOperations;
use kanban_service::api::CardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

impl ToolScoped for ArchiveCardsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            cards: self.cards.iter().map(|r| Ref::of(r)).collect(),
            ..Default::default()
        }
    }
}

impl ToolScoped for MoveCardsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            cards: self.cards.iter().map(|r| Ref::of(r)).collect(),
            column: Some(Ref::of(&self.column)),
            wants_board_columns: matches!(Ref::of(&self.column), Ref::Name),
            ..Default::default()
        }
    }
}

impl ToolScoped for AssignCardsToSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            cards: self.cards.iter().map(|r| Ref::of(r)).collect(),
            sprint: Some(Ref::of(&self.sprint)),
            wants_board_sprints: matches!(Ref::of(&self.sprint), Ref::Name),
            ..Default::default()
        }
    }
}

impl ToolScoped for AssignCardToSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.sprint)),
            wants_board_sprints: matches!(Ref::of(&self.sprint), Ref::Name),
            ..Default::default()
        }
    }
}

#[tool_router(router = card_batch_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    // Card Sprint Operations

    #[tool(description = "Assign a card to a sprint on the same board")]
    pub async fn tool_assign_card_to_sprint(
        &self,
        Parameters(req): Parameters<AssignCardToSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let card = locked_write(&self.ctx, |ctx| {
            let mut model = ctx.model_for(&scope);
            let card_id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let board_id = card_board(ctx, card_id)?;
            ctx.sync_into(&req.scope().for_board(board_id), &mut model);
            let sprint_id = resolve_sprint_in_board(&model, &req.sprint, board_id)?;
            ctx.assign_card_to_sprint(card_id, sprint_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    #[tool(description = "Unassign a card from its sprint")]
    pub async fn tool_unassign_card_from_sprint(
        &self,
        Parameters(req): Parameters<UnassignCardFromSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let card = locked_write(&self.ctx, |ctx| {
            let card_id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            ctx.unassign_card_from_sprint(card_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    // Multi-card operations

    #[tool(
        description = "Archive multiple cards at once. IDs may be UUIDs or identifiers (e.g. 'KAN-1', '42')."
    )]
    pub async fn tool_archive_cards(
        &self,
        Parameters(req): Parameters<ArchiveCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let count = locked_write(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let ids = resolve_cards(&model, &req.cards)?;
            ctx.archive_cards(ids).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"archived_count": count}))
    }

    #[tool(
        description = "Move multiple cards to a column. All cards must share a board; the column is resolved on that board."
    )]
    pub async fn tool_move_cards(
        &self,
        Parameters(req): Parameters<MoveCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let count = locked_write(&self.ctx, |ctx| {
            let mut model = ctx.model_for(&scope);
            let ids = resolve_cards(&model, &req.cards)?;
            let board_id = ctx.require_same_board(&ids).map_err(kanban_err_to_mcp)?;
            ctx.sync_into(&req.scope().for_board(board_id), &mut model);
            let column_id = resolve_column_in_board(&model, &req.column, board_id)?;
            ctx.move_cards(ids, column_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"moved_count": count}))
    }

    #[tool(
        description = "Assign multiple cards to a sprint. All cards must share a board; the sprint is resolved on that board."
    )]
    pub async fn tool_assign_cards_to_sprint(
        &self,
        Parameters(req): Parameters<AssignCardsToSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let count = locked_write(&self.ctx, |ctx| {
            let mut model = ctx.model_for(&scope);
            let ids = resolve_cards(&model, &req.cards)?;
            let board_id = ctx.require_same_board(&ids).map_err(kanban_err_to_mcp)?;
            ctx.sync_into(&req.scope().for_board(board_id), &mut model);
            let sprint_id = resolve_sprint_in_board(&model, &req.sprint, board_id)?;
            ctx.assign_cards_to_sprint(ids, sprint_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"assigned_count": count}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::board::CreateBoardRequest;
    use crate::requests::card::CreateCardParams;
    use crate::requests::column::CreateColumnParams;
    use crate::requests::sprint::CreateSprintParams;
    use crate::McpServer;
    use kanban_backend::{KanbanBackend, KanbanBackendFactory};
    use kanban_core::AppConfig;
    use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};
    use kanban_persistence_sqlite::{SqliteBackendFactory, SqliteStoreFactory};
    use kanban_service::test_helpers::FaultInjectingBackend;
    use rmcp::model::ErrorCode;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    struct RecordingFactory {
        inner: Box<dyn KanbanBackendFactory>,
        handle: Arc<Mutex<Option<Arc<FaultInjectingBackend>>>>,
    }

    #[async_trait::async_trait]
    impl KanbanBackendFactory for RecordingFactory {
        fn name(&self) -> &str {
            self.inner.name()
        }

        fn matches_locator(&self, locator: &str, header: &[u8]) -> bool {
            self.inner.matches_locator(locator, header)
        }

        async fn create(
            &self,
            locator: &str,
            config: &AppConfig,
        ) -> kanban_domain::KanbanResult<Arc<dyn KanbanBackend>> {
            let inner = self.inner.create(locator, config).await?;
            let wrapped = Arc::new(FaultInjectingBackend::new(inner));
            *self.handle.lock().unwrap() = Some(Arc::clone(&wrapped));
            Ok(wrapped as Arc<dyn KanbanBackend>)
        }
    }

    fn text_payload(result: &rmcp::model::CallToolResult) -> serde_json::Value {
        let raw = &result.content[0]
            .as_text()
            .expect("expected text content")
            .text;
        serde_json::from_str(raw).expect("tool result is JSON")
    }

    struct Seeded {
        server: KanbanMcpServer,
        _dir: TempDir,
        handle: Arc<FaultInjectingBackend>,
        card_id: String,
        card_identifier: String,
        sprint_id: String,
    }

    async fn seeded_server(file_name: &str) -> Seeded {
        let sqlite_handle = Arc::new(Mutex::new(None));
        let json_handle = Arc::new(Mutex::new(None));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(file_name);

        let server = McpServer::default()
            .register_backend(
                Box::new(SqliteStoreFactory),
                Box::new(RecordingFactory {
                    inner: Box::new(SqliteBackendFactory),
                    handle: Arc::clone(&sqlite_handle),
                }),
            )
            .register_backend(
                Box::new(JsonStoreFactory),
                Box::new(RecordingFactory {
                    inner: Box::new(JsonBackendFactory),
                    handle: Arc::clone(&json_handle),
                }),
            )
            .with_data_file(path.to_string_lossy().to_string())
            .build()
            .await
            .unwrap();

        let board = text_payload(
            &server
                .tool_create_board(Parameters(crate::requests::board::CreateBoardParams {
                    content: CreateBoardRequest {
                        id: None,
                        name: "Alpha".to_string(),
                        description: None,
                        sprint_prefix: None,
                        card_prefix: None,
                        task_sort_field: None,
                        task_sort_order: None,
                        sprint_duration_days: None,
                        task_list_view: None,
                    },
                    with_default_columns: None,
                }))
                .await
                .unwrap(),
        );
        let board_id = board["id"].as_str().unwrap().to_string();

        let column = text_payload(
            &server
                .tool_create_column(Parameters(CreateColumnParams {
                    board: board_id.clone(),
                    content: kanban_service::api::CreateColumnRequest {
                        id: None,
                        name: "TODO".to_string(),
                        wip_limit: None,
                        default_status: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let column_id = column["id"].as_str().unwrap().to_string();

        let sprint = text_payload(
            &server
                .tool_create_sprint(Parameters(CreateSprintParams {
                    board: board_id.clone(),
                    content: kanban_service::api::CreateSprintRequest {
                        id: None,
                        name: Some("Sprint 1".to_string()),
                        prefix: None,
                        card_prefix: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let sprint_id = sprint["id"].as_str().unwrap().to_string();

        let card = text_payload(
            &server
                .tool_create_card(Parameters(CreateCardParams {
                    board: board_id.clone(),
                    column: column_id.clone(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Do the thing".to_string(),
                        description: None,
                        priority: None,
                        due_date: None,
                        points: None,
                        sprint_id: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let card_id = card["id"].as_str().unwrap().to_string();
        let card_identifier = format!(
            "{}-{}",
            card["prefix"].as_str().unwrap(),
            card["card_number"].as_u64().unwrap()
        );

        let handle = sqlite_handle
            .lock()
            .unwrap()
            .clone()
            .or_else(|| json_handle.lock().unwrap().clone())
            .expect("a backend must have been created");

        Seeded {
            server,
            _dir: dir,
            handle,
            card_id,
            card_identifier,
            sprint_id,
        }
    }

    #[tokio::test]
    async fn test_archive_cards_by_uuid_does_not_read_the_card_list_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_archive_cards(Parameters(ArchiveCardsRequest {
                    cards: vec![seeded.card_id.clone()],
                }))
                .await
                .unwrap(),
        );

        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);
        assert_eq!(response["archived_count"], 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_archive_cards_by_uuid_does_not_read_the_card_list_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_archive_cards(Parameters(ArchiveCardsRequest {
                    cards: vec![seeded.card_id.clone()],
                }))
                .await
                .unwrap(),
        );

        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);
        assert_eq!(response["archived_count"], 1);
    }

    #[tokio::test]
    async fn test_archive_cards_with_an_unloadable_card_list_errors_naming_the_collection_on_json()
    {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_cards");

        let err = seeded
            .server
            .tool_archive_cards(Parameters(ArchiveCardsRequest {
                cards: vec!["KAN-1".to_string()],
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("card list"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_archive_cards_with_an_unloadable_card_list_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_cards");

        let err = seeded
            .server
            .tool_archive_cards(Parameters(ArchiveCardsRequest {
                cards: vec!["KAN-1".to_string()],
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("card list"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_move_cards_with_an_unloadable_column_collection_errors_naming_the_collection_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_move_cards(Parameters(MoveCardsRequest {
                cards: vec![seeded.card_id.clone()],
                column: "TODO".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_move_cards_with_an_unloadable_column_collection_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_move_cards(Parameters(MoveCardsRequest {
                cards: vec![seeded.card_id.clone()],
                column: "TODO".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_assign_card_to_sprint_with_an_unloadable_sprint_collection_errors_naming_the_collection_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_assign_card_to_sprint(Parameters(AssignCardToSprintRequest {
                card: seeded.card_id.clone(),
                sprint: "Sprint 1".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
        let _ = &seeded.sprint_id;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_assign_card_to_sprint_with_an_unloadable_sprint_collection_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_assign_card_to_sprint(Parameters(AssignCardToSprintRequest {
                card: seeded.card_id.clone(),
                sprint: "Sprint 1".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
        let _ = &seeded.sprint_id;
    }

    #[tokio::test]
    async fn test_assign_cards_to_sprint_by_uuid_assigns_the_card_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_assign_cards_to_sprint(Parameters(AssignCardsToSprintRequest {
                    cards: vec![seeded.card_id.clone()],
                    sprint: "Sprint 1".to_string(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["assigned_count"], 1);
        let _ = &seeded.sprint_id;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_assign_cards_to_sprint_by_uuid_assigns_the_card_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_assign_cards_to_sprint(Parameters(AssignCardsToSprintRequest {
                    cards: vec![seeded.card_id.clone()],
                    sprint: "Sprint 1".to_string(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["assigned_count"], 1);
        let _ = &seeded.sprint_id;
    }

    #[tokio::test]
    async fn test_assign_cards_to_sprint_by_name_assigns_the_card_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_assign_cards_to_sprint(Parameters(AssignCardsToSprintRequest {
                    cards: vec![seeded.card_identifier.clone()],
                    sprint: "Sprint 1".to_string(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["assigned_count"], 1);
        let _ = &seeded.sprint_id;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_assign_cards_to_sprint_by_name_assigns_the_card_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_assign_cards_to_sprint(Parameters(AssignCardsToSprintRequest {
                    cards: vec![seeded.card_identifier.clone()],
                    sprint: "Sprint 1".to_string(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["assigned_count"], 1);
        let _ = &seeded.sprint_id;
    }

    #[tokio::test]
    async fn test_archive_cards_by_name_archives_the_card_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_archive_cards(Parameters(ArchiveCardsRequest {
                    cards: vec![seeded.card_identifier.clone()],
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["archived_count"], 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_archive_cards_by_name_archives_the_card_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        let response = text_payload(
            &seeded
                .server
                .tool_archive_cards(Parameters(ArchiveCardsRequest {
                    cards: vec![seeded.card_identifier.clone()],
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["archived_count"], 1);
    }

    #[tokio::test]
    async fn test_assign_cards_to_sprint_with_an_unloadable_sprint_collection_errors_naming_the_collection_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_assign_cards_to_sprint(Parameters(AssignCardsToSprintRequest {
                cards: vec![seeded.card_id.clone()],
                sprint: "Sprint 1".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_assign_cards_to_sprint_with_an_unloadable_sprint_collection_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_assign_cards_to_sprint(Parameters(AssignCardsToSprintRequest {
                cards: vec![seeded.card_id.clone()],
                sprint: "Sprint 1".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }
}
