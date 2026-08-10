use kanban_core::AppConfig;
use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_mcp::context::McpContext;
use kanban_service::{KanbanContext, StoreManager};
use tempfile::TempDir;

fn default_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(registry, backends)
}

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let sm = default_store_manager();
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

async fn setup() -> (McpContext, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let path = dir.path().join("test.json");
    let path_str = path.to_string_lossy().to_string();
    let store_manager = default_store_manager();
    let ctx = McpContext::new(&store_manager, &path_str, AppConfig::default())
        .await
        .unwrap();
    (ctx, dir)
}

// Board round-trips

#[tokio::test]
async fn board_create_list_get() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx
        .create_board("Test Board".into(), Some("TB".into()))
        .unwrap();
    assert_eq!(board.name, "Test Board");

    let boards = ctx.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board.id);

    let fetched = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(fetched.name, "Test Board");
}

/// Opening a future-format JSON file via the MCP surface must surface as
/// `McpError::invalid_params`, not `internal_error`. The data file the client
/// pointed at is the precondition that failed — that's the same category as
/// any other invalid argument. Without this mapping, an LLM client sees
/// "internal error" for what is fundamentally "your file is too new for this
/// binary" and has no way to suggest the right fix.
#[tokio::test]
async fn open_future_version_file_returns_invalid_params() {
    use kanban_mcp::error::KanbanMcpError;
    use rmcp::model::ErrorCode;
    use serde_json::json;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("future.json");
    let v99 = json!({
        "version": 99,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2030-01-01T00:00:00Z"
        },
        "data": {}
    });
    std::fs::write(&path, v99.to_string()).unwrap();

    let store_manager = default_store_manager();
    let err = McpContext::new(
        &store_manager,
        &path.to_string_lossy(),
        AppConfig::default(),
    )
    .await
    .err()
    .expect("v99 file must be refused");

    // KanbanError → KanbanMcpError → McpError (rmcp::model::ErrorData) is the
    // path every MCP tool handler walks via `?`. We follow it here to pin the
    // wire-level error_code, not just the Rust variant.
    let mcp_err: rmcp::model::ErrorData = KanbanMcpError::Domain(err).into();
    assert_eq!(
        mcp_err.code,
        ErrorCode::INVALID_PARAMS,
        "UnsupportedFutureVersion must map to INVALID_PARAMS, got: {mcp_err:?}"
    );
    assert!(
        mcp_err.message.contains("upgrade kanban"),
        "error message must include the upgrade hint, got: {}",
        mcp_err.message
    );
}

/// SQLite parallel of the JSON future-version MCP test above. Same wire-level
/// expectation: `ErrorCode::INVALID_PARAMS`, message mentions "upgrade kanban".
#[tokio::test(flavor = "multi_thread")]
async fn open_future_version_sqlite_file_returns_invalid_params() {
    use kanban_mcp::error::KanbanMcpError;
    use rmcp::model::ErrorCode;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("future.db");
    kanban_persistence_sqlite::write_test_metadata_with_schema_version(&path, 99)
        .await
        .unwrap();

    let store_manager = default_store_manager();
    let err = McpContext::new(
        &store_manager,
        &path.to_string_lossy(),
        AppConfig::default(),
    )
    .await
    .err()
    .expect("v99 SQLite DB must be refused");

    let mcp_err: rmcp::model::ErrorData = KanbanMcpError::Domain(err).into();
    assert_eq!(
        mcp_err.code,
        ErrorCode::INVALID_PARAMS,
        "UnsupportedFutureVersion (sqlite path) must map to INVALID_PARAMS, got: {mcp_err:?}"
    );
    assert!(
        mcp_err.message.contains("upgrade kanban"),
        "error message must include the upgrade hint, got: {}",
        mcp_err.message
    );
}

#[tokio::test]
async fn board_get_nonexistent() {
    let (ctx, _tmp) = setup().await;
    let id = uuid::Uuid::new_v4();
    let result = ctx.get_board(id).unwrap();
    assert!(result.is_none());
}

// Column round-trips

#[tokio::test]
async fn column_create_list_update() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "To Do".into(), None).unwrap();
    assert_eq!(col.name, "To Do");

    let cols = ctx.list_columns(board.id).unwrap();
    assert!(cols.iter().any(|c| c.id == col.id));

    let updated = ctx
        .update_column(
            col.id,
            kanban_domain::ColumnUpdate {
                name: Some("Done".into()),
                position: None,
                wip_limit: kanban_domain::FieldUpdate::NoChange,
            },
        )
        .unwrap();
    assert_eq!(updated.name, "Done");
}

#[tokio::test]
async fn column_reorder() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let _c1 = ctx
        .create_column(board.id, "Col A".into(), Some(0))
        .unwrap();
    let c2 = ctx
        .create_column(board.id, "Col B".into(), Some(1))
        .unwrap();
    let reordered = ctx.reorder_column(c2.id, 0).unwrap();
    assert_eq!(reordered.position, 0);
}

// Card round-trips

#[tokio::test]
async fn card_create_get_move_archive_restore() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col1 = ctx.create_column(board.id, "To Do".into(), None).unwrap();
    let col2 = ctx.create_column(board.id, "Done".into(), None).unwrap();

    let card = ctx
        .create_card(board.id, col1.id, "My Card".into(), Default::default())
        .unwrap();
    assert_eq!(card.title, "My Card");

    let fetched = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(fetched.id, card.id);

    let moved = ctx.move_card(card.id, col2.id, None).unwrap();
    assert_eq!(moved.column_id, col2.id);

    ctx.archive_card(card.id).unwrap();
    let archived = ctx.list_archived_cards().unwrap();
    assert!(archived.iter().any(|c| c.entity_id == card.id));

    let restored = ctx.restore_card(card.id, None).unwrap();
    assert_eq!(restored.id, card.id);
}

#[tokio::test]
async fn create_card_then_update_with_all_fields() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "To Do".into(), None).unwrap();

    let card = ctx
        .create_card(board.id, col.id, "Full Card".into(), Default::default())
        .unwrap();
    assert_eq!(card.title, "Full Card");

    let updated = ctx
        .update_card(
            card.id,
            kanban_domain::CardUpdate {
                title: None,
                description: kanban_domain::FieldUpdate::Set("A description".into()),
                priority: Some(kanban_domain::CardPriority::High),
                status: None,
                position: None,
                column_id: None,
                points: kanban_domain::FieldUpdate::Set(5),
                due_date: kanban_domain::FieldUpdate::NoChange,
                sprint_id: kanban_domain::FieldUpdate::NoChange,
            },
        )
        .unwrap();
    assert_eq!(updated.title, "Full Card");
    assert_eq!(updated.description.as_deref(), Some("A description"));
}

// KAN-394: status ↔ completion column invariant via MCP

#[tokio::test]
async fn mcp_update_card_status_to_done_moves_to_completion_column() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let backlog = ctx.create_column(board.id, "Backlog".into(), None).unwrap();
    let _progress = ctx
        .create_column(board.id, "In Progress".into(), None)
        .unwrap();
    let done = ctx.create_column(board.id, "Done".into(), None).unwrap();

    let card = ctx
        .create_card(board.id, backlog.id, "Card".into(), Default::default())
        .unwrap();

    let updated = ctx
        .update_card(
            card.id,
            kanban_domain::CardUpdate {
                status: Some(kanban_domain::CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated.status, kanban_domain::CardStatus::Done);
    assert_eq!(
        updated.column_id, done.id,
        "MCP update_card(status=Done) must move card to completion column"
    );
    assert!(updated.completed_at.is_some());
}

#[tokio::test]
async fn mcp_move_card_to_completion_column_sets_status_done() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let backlog = ctx.create_column(board.id, "Backlog".into(), None).unwrap();
    let _progress = ctx
        .create_column(board.id, "In Progress".into(), None)
        .unwrap();
    let done = ctx.create_column(board.id, "Done".into(), None).unwrap();

    let card = ctx
        .create_card(board.id, backlog.id, "Card".into(), Default::default())
        .unwrap();

    let moved = ctx.move_card(card.id, done.id, None).unwrap();
    assert_eq!(moved.column_id, done.id);
    assert_eq!(
        moved.status,
        kanban_domain::CardStatus::Done,
        "MCP move_card to completion column must set status=Done"
    );
    assert!(moved.completed_at.is_some());
}

#[tokio::test]
async fn mcp_move_card_away_from_completion_column_clears_done_status() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let backlog = ctx.create_column(board.id, "Backlog".into(), None).unwrap();
    let _progress = ctx
        .create_column(board.id, "In Progress".into(), None)
        .unwrap();
    let done = ctx.create_column(board.id, "Done".into(), None).unwrap();

    let card = ctx
        .create_card(board.id, backlog.id, "Card".into(), Default::default())
        .unwrap();

    // Send card to Done via MCP move
    let _ = ctx.move_card(card.id, done.id, None).unwrap();

    // Now move it back to Backlog — status must clear
    let moved_back = ctx.move_card(card.id, backlog.id, None).unwrap();
    assert_eq!(moved_back.column_id, backlog.id);
    assert_eq!(
        moved_back.status,
        kanban_domain::CardStatus::Todo,
        "MCP move_card away from completion column must clear Done status"
    );
    assert!(moved_back.completed_at.is_none());
}

// Sprint round-trips

#[tokio::test]
async fn sprint_create_list_activate_complete() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();

    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let sprints = ctx.list_sprints(board.id).unwrap();
    assert_eq!(sprints.len(), 1);
    assert_eq!(sprints[0].id, sprint.id);

    let activated = ctx.activate_sprint(sprint.id, Some(14)).unwrap();
    assert_eq!(activated.id, sprint.id);

    let completed = ctx.complete_sprint(sprint.id).unwrap();
    assert_eq!(completed.id, sprint.id);
}

#[tokio::test]
async fn sprint_update_via_trait() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let updated = ctx
        .update_sprint(
            sprint.id,
            kanban_domain::SprintUpdate {
                name: Some("Sprint Alpha".into()),
                name_index: kanban_domain::FieldUpdate::NoChange,
                prefix: kanban_domain::FieldUpdate::Set("SA".into()),
                card_prefix: kanban_domain::FieldUpdate::NoChange,
                status: None,
                start_date: kanban_domain::FieldUpdate::Set(
                    chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc(),
                ),
                end_date: kanban_domain::FieldUpdate::Set(
                    chrono::NaiveDate::from_ymd_opt(2025, 1, 15)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc(),
                ),
            },
        )
        .unwrap();
    assert_eq!(updated.id, sprint.id);
}

#[tokio::test]
async fn sprint_cancel() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let _ = ctx.activate_sprint(sprint.id, None).unwrap();
    let cancelled = ctx.cancel_sprint(sprint.id).unwrap();
    assert_eq!(cancelled.id, sprint.id);
}

// Card-sprint assignment

#[tokio::test]
async fn card_assign_unassign_sprint() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "To Do".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "Card".into(), Default::default())
        .unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let assigned = ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    assert_eq!(assigned.sprint_id, Some(sprint.id));

    let unassigned = ctx.unassign_card_from_sprint(card.id).unwrap();
    assert_eq!(unassigned.sprint_id, None);
}

// Multi-card operations

#[tokio::test]
async fn archive_cards() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let c1 = ctx
        .create_card(board.id, col.id, "Card 1".into(), Default::default())
        .unwrap();
    let c2 = ctx
        .create_card(board.id, col.id, "Card 2".into(), Default::default())
        .unwrap();

    let count = ctx.archive_cards(vec![c1.id, c2.id]).unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn move_cards() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col1 = ctx.create_column(board.id, "From".into(), None).unwrap();
    let col2 = ctx.create_column(board.id, "To".into(), None).unwrap();
    let c1 = ctx
        .create_card(board.id, col1.id, "Card 1".into(), Default::default())
        .unwrap();
    let c2 = ctx
        .create_card(board.id, col1.id, "Card 2".into(), Default::default())
        .unwrap();

    let count = ctx.move_cards(vec![c1.id, c2.id], col2.id).unwrap();
    assert_eq!(count, 2);
}

// Export/Import round-trip

#[tokio::test]
async fn export_import_roundtrip() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("Export Board".into(), None).unwrap();
    let _col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let json = ctx.export_board(Some(board.id)).unwrap();
    assert!(json.contains("Export Board"));

    // Import into a fresh context to avoid duplicate UUID errors
    let (mut ctx2, _tmp2) = setup().await;
    let imported = ctx2.import_board(&json).unwrap();
    assert_eq!(imported.name, "Export Board");
}

// Persistence round-trips

#[tokio::test]
async fn test_create_board_persists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    let path_str = path.to_string_lossy().to_string();

    let store_manager = default_store_manager();
    let mut mcp_ctx = McpContext::new(&store_manager, &path_str, AppConfig::default())
        .await
        .unwrap();
    mcp_ctx
        .create_board("Persistent Board".into(), None)
        .unwrap();
    mcp_ctx.save().await.unwrap();

    let fresh = open_context(&path_str, AppConfig::default()).await.unwrap();
    let boards = fresh.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Persistent Board");
}

#[tokio::test]
async fn test_mutation_sequence_persists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    let path_str = path.to_string_lossy().to_string();

    let store_manager = default_store_manager();
    let mut mcp_ctx = McpContext::new(&store_manager, &path_str, AppConfig::default())
        .await
        .unwrap();
    let board = mcp_ctx.create_board("Board".into(), None).unwrap();
    let col = mcp_ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    mcp_ctx
        .create_card(board.id, col.id, "Task".into(), Default::default())
        .unwrap();
    mcp_ctx.save().await.unwrap();

    let fresh = open_context(&path_str, AppConfig::default()).await.unwrap();
    assert_eq!(fresh.list_boards().unwrap().len(), 1);
    assert_eq!(fresh.list_columns(board.id).unwrap().len(), 1);
    assert_eq!(
        fresh
            .list_cards(kanban_domain::CardListFilter::default())
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn test_delete_persists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    let path_str = path.to_string_lossy().to_string();

    let store_manager = default_store_manager();
    let mut mcp_ctx = McpContext::new(&store_manager, &path_str, AppConfig::default())
        .await
        .unwrap();
    let board = mcp_ctx.create_board("Temp Board".into(), None).unwrap();
    mcp_ctx.save().await.unwrap();

    mcp_ctx.delete_board(board.id).unwrap();
    mcp_ctx.save().await.unwrap();

    let fresh = open_context(&path_str, AppConfig::default()).await.unwrap();
    assert!(fresh.list_boards().unwrap().is_empty());
}

// find_cards_by_identifier

#[tokio::test]
async fn find_cards_by_identifier_single_match() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx
        .create_board("Project".into(), Some("KAN".into()))
        .unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "My Task".into(), Default::default())
        .unwrap();

    let results = ctx.find_cards_by_identifier("KAN-1").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, card.id);
}

#[tokio::test]
async fn find_cards_by_identifier_multiple_matches() {
    let (mut ctx, _tmp) = setup().await;

    let board_a = ctx
        .create_board("Board A".into(), Some("KAN".into()))
        .unwrap();
    let col_a = ctx.create_column(board_a.id, "Todo".into(), None).unwrap();
    let card_a = ctx
        .create_card(board_a.id, col_a.id, "Card on A".into(), Default::default())
        .unwrap();

    let board_b = ctx
        .create_board("Board B".into(), Some("KAN".into()))
        .unwrap();
    let col_b = ctx.create_column(board_b.id, "Todo".into(), None).unwrap();
    let card_b = ctx
        .create_card(board_b.id, col_b.id, "Card on B".into(), Default::default())
        .unwrap();

    let results = ctx.find_cards_by_identifier("KAN-1").unwrap();
    assert_eq!(results.len(), 2);
    let ids: Vec<_> = results.iter().map(|c| c.id).collect();
    assert!(ids.contains(&card_a.id));
    assert!(ids.contains(&card_b.id));
}

#[tokio::test]
async fn find_cards_by_identifier_not_found() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx
        .create_board("Project".into(), Some("KAN".into()))
        .unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    ctx.create_card(board.id, col.id, "My Task".into(), Default::default())
        .unwrap();

    let results = ctx.find_cards_by_identifier("KAN-99").unwrap();
    assert!(results.is_empty());
}

// Undo/Redo

#[tokio::test]
async fn test_mcp_undo_reverses_create_board() {
    let (mut ctx, _tmp) = setup().await;
    ctx.create_board("Board".into(), None).unwrap();
    assert_eq!(ctx.list_boards().unwrap().len(), 1);

    assert!(ctx.undo().unwrap());
    assert!(ctx.list_boards().unwrap().is_empty());
}

#[tokio::test]
async fn test_mcp_redo_restores_undone_board() {
    let (mut ctx, _tmp) = setup().await;
    ctx.create_board("Board".into(), None).unwrap();
    ctx.undo().unwrap();
    assert!(ctx.list_boards().unwrap().is_empty());

    assert!(ctx.redo().unwrap());
    assert_eq!(ctx.list_boards().unwrap().len(), 1);
}

#[tokio::test]
async fn test_mcp_undo_on_empty_returns_false() {
    let (mut ctx, _tmp) = setup().await;
    assert!(!ctx.can_undo());
    assert!(!ctx.undo().unwrap());
}

#[tokio::test]
async fn test_mcp_reload_resets_undo_history() {
    // reload() semantics: "pick up external changes". The previous undo history
    // was computed against a different file state and is no longer valid.
    let (mut ctx, _tmp) = setup().await;
    ctx.create_board("Board".into(), None).unwrap();
    assert!(ctx.can_undo(), "should have undo entry after create");
    ctx.save().await.unwrap();
    ctx.reload().await.unwrap();
    assert!(
        !ctx.can_undo(),
        "reload must reset undo history — cursor is invalid after external change"
    );
}

// ============================================================================
// Name resolution via McpContext (same default trait methods as CLI uses).
// Confirms the MCP context picks up the shared resolvers and they produce
// the same human-friendly error messages.
// ============================================================================

#[tokio::test]
async fn resolve_board_id_by_name_on_mcp_context() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx
        .create_board("MyBoard".into(), Some("MB".into()))
        .unwrap();
    assert_eq!(ctx.resolve_board_id("MyBoard").unwrap(), board.id);
    assert_eq!(ctx.resolve_board_id("myboard").unwrap(), board.id);
}

#[tokio::test]
async fn resolve_board_id_unknown_lists_available_on_mcp() {
    let (mut ctx, _tmp) = setup().await;
    ctx.create_board("Alpha".into(), None).unwrap();
    ctx.create_board("Beta".into(), None).unwrap();
    let msg = ctx.resolve_board_id("Gamma").unwrap_err().to_string();
    assert!(msg.contains("not found"), "msg: {msg}");
    assert!(msg.contains("'Alpha'"), "msg: {msg}");
    assert!(msg.contains("'Beta'"), "msg: {msg}");
}

#[tokio::test]
async fn resolve_column_id_global_ambiguous_on_mcp() {
    let (mut ctx, _tmp) = setup().await;
    let a = ctx.create_board("A".into(), None).unwrap();
    let b = ctx.create_board("B".into(), None).unwrap();
    ctx.create_column(a.id, "TODO".into(), None).unwrap();
    ctx.create_column(b.id, "TODO".into(), None).unwrap();
    let msg = ctx
        .resolve_column_id_global("todo")
        .unwrap_err()
        .to_string();
    assert!(msg.contains("ambiguous"), "msg: {msg}");
    assert!(msg.contains("'A'"), "msg: {msg}");
    assert!(msg.contains("'B'"), "msg: {msg}");
}

#[tokio::test]
async fn resolve_sprint_id_by_name_and_number_on_mcp() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    let sprint = ctx
        .create_sprint(board.id, None, Some("alpha".into()))
        .unwrap();
    assert_eq!(ctx.resolve_sprint_id("alpha", board.id).unwrap(), sprint.id);
    assert_eq!(
        ctx.resolve_sprint_id(&sprint.sprint_number.to_string(), board.id)
            .unwrap(),
        sprint.id
    );
}

#[tokio::test]
async fn resolve_card_ids_aggregates_failures_on_mcp() {
    let (mut ctx, _tmp) = setup().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "T".into(), Default::default())
        .unwrap();
    let raws = vec![format!("KAN-{}", card.card_number), "KAN-999".into()];
    let err = ctx.resolve_card_ids(&raws).unwrap_err().to_string();
    assert!(err.contains("KAN-999"), "msg: {err}");
}

#[tokio::test]
async fn require_same_board_rejects_cross_board_on_mcp() {
    let (mut ctx, _tmp) = setup().await;
    let a = ctx.create_board("Alpha".into(), Some("A".into())).unwrap();
    let b = ctx.create_board("Beta".into(), Some("B".into())).unwrap();
    let col_a = ctx.create_column(a.id, "TODO".into(), None).unwrap();
    let col_b = ctx.create_column(b.id, "TODO".into(), None).unwrap();
    let c_a = ctx
        .create_card(a.id, col_a.id, "a".into(), Default::default())
        .unwrap();
    let c_b = ctx
        .create_card(b.id, col_b.id, "b".into(), Default::default())
        .unwrap();
    let err = ctx
        .require_same_board(&[c_a.id, c_b.id])
        .unwrap_err()
        .to_string();
    assert!(err.contains("same board"), "msg: {err}");
    assert!(err.contains("'Alpha'"), "msg: {err}");
    assert!(err.contains("'Beta'"), "msg: {err}");
}

// ============================================================================
// Tool-handler tests (KAN-400 review fix: previously only McpContext was tested,
// not the actual tool bodies that go through `locked_session` + resolution).
// ============================================================================

use kanban_mcp::{
    ArchiveBoardRequest, AssignCardToSprintRequest, CarryOverSprintCardsRequest,
    CreateBoardRequest, CreateCardParams, CreateColumnParams, CreateSprintParams,
    DeleteArchivedBoardRequest, GetBoardRequest, GetCardRequest, GetColumnRequest,
    GetSprintRequest, KanbanMcpServer, ListBoardsRequest, ListColumnsRequest, ListSprintsRequest,
    MoveCardRequest, MoveCardsRequest, RestoreBoardRequest,
};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;

async fn setup_server() -> (KanbanMcpServer, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    let store_manager = default_store_manager();
    let server = KanbanMcpServer::new(
        &store_manager,
        &path.to_string_lossy(),
        AppConfig::default(),
    )
    .await
    .unwrap();
    (server, dir)
}

/// Minimal-path board-create request: just name + card_prefix, the only fields
/// these seed-helpers need. The shared `CreateBoardRequest` carries the full
/// create spec; the remaining fields default to `None`.
fn board_req(name: &str, card_prefix: Option<String>) -> CreateBoardRequest {
    CreateBoardRequest {
        id: None,
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    }
}

/// Minimal-path column-create request (KAN-794): the `board` name plus the
/// shared `kanban_service::api::CreateColumnRequest` content with no client id
/// and no wip_limit. Position is server-assigned on append (not a create field).
fn column_req(board: &str, name: &str) -> CreateColumnParams {
    CreateColumnParams {
        board: board.to_string(),
        content: kanban_service::api::CreateColumnRequest {
            id: None,
            name: name.to_string(),
            wip_limit: None,
        },
    }
}

/// Minimal-path sprint-create request (KAN-798): the `board` name-or-id plus
/// the shared `kanban_service::api::CreateSprintRequest` content carrying just a
/// `name`. No client id, no explicit prefix, no card_prefix. The MCP create tool
/// resolves the board, then funnels this through `create_sprint_from_spec`.
fn sprint_req(board: &str, name: &str) -> CreateSprintParams {
    CreateSprintParams {
        board: board.to_string(),
        content: kanban_service::api::CreateSprintRequest {
            id: None,
            name: Some(name.to_string()),
            prefix: None,
            card_prefix: None,
        },
    }
}

fn text_payload(result: &rmcp::model::CallToolResult) -> Value {
    let raw = &result.content[0]
        .as_text()
        .expect("expected text content")
        .text;
    serde_json::from_str(raw).expect("tool result is JSON")
}

#[tokio::test]
async fn tool_move_card_resolves_names_through_locked_session() {
    let (server, _tmp) = setup_server().await;
    // Seed: board with two columns and one card.
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "Doing")))
        .await
        .unwrap();
    server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: None,
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "T".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    // Move KAN-1 to Doing using names end-to-end.
    let result = server
        .tool_move_card(Parameters(MoveCardRequest {
            card: "KAN-1".into(),
            column: "Doing".into(),
            position: None,
        }))
        .await
        .unwrap();
    let body = text_payload(&result);
    assert_eq!(body["title"], "T");
    assert!(body["column_id"].is_string());
}

#[tokio::test]
async fn tool_move_cards_rejects_cross_board_batch() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
        .await
        .unwrap();
    server
        .tool_create_board(Parameters(board_req("Beta", Some("B".into()))))
        .await
        .unwrap();
    for board in ["Alpha", "Beta"] {
        server
            .tool_create_column(Parameters(column_req(board, "TODO")))
            .await
            .unwrap();
        server
            .tool_create_card(Parameters(CreateCardParams {
                board: board.into(),
                column: "TODO".into(),
                sprint: None,
                content: kanban_service::api::CreateCardRequest {
                    id: None,
                    title: format!("{board}-1"),
                    description: None,
                    priority: None,
                    due_date: None,
                    points: None,
                    sprint_id: None,
                },
            }))
            .await
            .unwrap();
    }
    let err = server
        .tool_move_cards(Parameters(MoveCardsRequest {
            cards: vec!["A-1".into(), "B-1".into()],
            column: "TODO".into(),
        }))
        .await
        .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("same board"), "err: {msg}");
    assert!(msg.contains("'Alpha'"), "err: {msg}");
    assert!(msg.contains("'Beta'"), "err: {msg}");
}

#[tokio::test]
async fn tool_carry_over_sprint_cards_scopes_to_named_from_board() {
    // from_sprint is global; to_sprint must resolve on from_sprint's board.
    // A sprint of the same name on a different board must not match `to`.
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
        .await
        .unwrap();
    server
        .tool_create_board(Parameters(board_req("Beta", Some("B".into()))))
        .await
        .unwrap();
    // Both boards get a "next" sprint name. Only Alpha gets the "completed" one.
    server
        .tool_create_sprint(Parameters(sprint_req("Alpha", "completed")))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("Alpha", "next")))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("Beta", "next")))
        .await
        .unwrap();
    // Activate + complete the source sprint on Alpha.
    server
        .tool_activate_sprint(Parameters(kanban_mcp::ActivateSprintRequest {
            sprint: "completed".into(),
            duration_days: Some(1),
        }))
        .await
        .unwrap();
    server
        .tool_complete_sprint(Parameters(kanban_mcp::CompleteSprintRequest {
            sprint: "completed".into(),
        }))
        .await
        .unwrap();
    // Even though both boards have a "next" sprint, the carry-over must
    // resolve "next" within Alpha (the source's board), not error on
    // ambiguity. (Per KAN-400 design: to_sprint is scoped to from_sprint's board.)
    let result = server
        .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
            from_sprint: "completed".into(),
            to_sprint: "next".into(),
        }))
        .await
        .unwrap();
    let body = text_payload(&result);
    assert!(body["carried_over_count"].is_number());
}

#[tokio::test]
async fn tool_assign_card_to_sprint_resolves_by_name_then_mutates() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("B", "alpha")))
        .await
        .unwrap();
    server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: None,
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "T".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    // Assign using card identifier + sprint name + sprint number both work.
    let r1 = server
        .tool_assign_card_to_sprint(Parameters(AssignCardToSprintRequest {
            card: "KAN-1".into(),
            sprint: "alpha".into(),
        }))
        .await
        .unwrap();
    let body = text_payload(&r1);
    assert!(body["sprint_id"].is_string());
    let r2 = server
        .tool_assign_card_to_sprint(Parameters(AssignCardToSprintRequest {
            card: "KAN-1".into(),
            sprint: "1".into(), // sprint number
        }))
        .await
        .unwrap();
    let body2 = text_payload(&r2);
    assert!(body2["sprint_id"].is_string());
}

// ============================================================================
// Card-relation tool surface (KAN-504).
// ============================================================================

use kanban_mcp::{
    ListCardChildrenRequest, ListCardParentsRequest, RemoveCardParentRequest, SetCardParentRequest,
};

async fn setup_server_with_two_cards() -> (KanbanMcpServer, TempDir, String, String) {
    let (server, dir) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: None,
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Parent".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: None,
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Child".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    (server, dir, "KAN-1".to_string(), "KAN-2".to_string())
}

#[tokio::test]
async fn tool_set_card_parent_resolves_identifiers_and_persists() {
    let (server, _tmp, parent, child) = setup_server_with_two_cards().await;

    let r = server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: child.clone(),
            parent: parent.clone(),
        }))
        .await
        .unwrap();
    let body = text_payload(&r);
    assert!(body["parent"].is_string());
    assert!(body["child"].is_string());

    let listed = server
        .tool_list_card_parents(Parameters(ListCardParentsRequest {
            card: child.clone(),
            page: None,
            page_size: None,
        }))
        .await
        .unwrap();
    let listed_body = text_payload(&listed);
    let parents = listed_body["items"].as_array().expect("items array");
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0]["title"], "Parent");
}

#[tokio::test]
async fn tool_set_card_parent_cycle_returns_mcp_error() {
    use rmcp::model::ErrorCode;

    let (server, _tmp, a, b) = setup_server_with_two_cards().await;

    server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: b.clone(),
            parent: a.clone(),
        }))
        .await
        .unwrap();

    // Closing the cycle b -> a should fail at the MCP boundary.
    let err = server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: a.clone(),
            parent: b.clone(),
        }))
        .await
        .unwrap_err();

    // KanbanMcpError maps domain errors (which DependencyError::CycleDetected
    // is) to INVALID_PARAMS at the boundary. Pin the JSON-RPC code so the
    // contract is not just stringly typed, and verify the cycle is the
    // source by inspecting the (typed) message.
    assert_eq!(
        err.code,
        ErrorCode::INVALID_PARAMS,
        "domain errors must surface as INVALID_PARAMS at the MCP boundary"
    );
    assert!(
        err.message.contains("cycle"),
        "message should mention cycle; got: {}",
        err.message
    );
    // The MCP boundary enriches cycle errors with the raw user
    // identifiers, same as CLI. Pin both sides of the edge so the
    // shared message formatter stays load-bearing across surfaces.
    assert!(
        err.message.contains(&a) && err.message.contains(&b),
        "cycle message should name both cards; got: {}",
        err.message
    );
}

/// Self-reference at the MCP boundary surfaces as INVALID_PARAMS and
/// names the offending card, matching the CLI UX. Pins the shared
/// enrichment path on the self-ref branch (the cycle test pins the
/// cycle branch).
#[tokio::test]
async fn tool_set_card_parent_self_reference_returns_invalid_params_with_card_identifier() {
    use rmcp::model::ErrorCode;

    let (server, _tmp, a, _b) = setup_server_with_two_cards().await;

    let err = server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: a.clone(),
            parent: a.clone(),
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    let msg = err.message.to_lowercase();
    assert!(
        msg.contains("self"),
        "self-reference message must name the invariant; got: {}",
        err.message
    );
    assert!(
        err.message.contains(&a),
        "self-reference message must name the offending card; got: {}",
        err.message
    );
}

#[tokio::test]
async fn tool_list_card_parents_returns_summaries() {
    let (server, _tmp, parent, child) = setup_server_with_two_cards().await;

    server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: child.clone(),
            parent: parent.clone(),
        }))
        .await
        .unwrap();

    let listed = server
        .tool_list_card_parents(Parameters(ListCardParentsRequest {
            card: child.clone(),
            page: None,
            page_size: None,
        }))
        .await
        .unwrap();
    let arr = text_payload(&listed);
    let parents = arr["items"].as_array().expect("items array");
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0]["title"], "Parent");
    assert!(parents[0]["id"].is_string());

    let children = server
        .tool_list_card_children(Parameters(ListCardChildrenRequest {
            card: parent.clone(),
            page: None,
            page_size: None,
        }))
        .await
        .unwrap();
    let arr = text_payload(&children);
    let cs = arr["items"].as_array().expect("items array");
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0]["title"], "Child");
}

#[tokio::test]
async fn tool_list_card_parents_and_children_return_paginated_envelope() {
    let (server, _tmp, parent, child) = setup_server_with_two_cards().await;

    server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: child.clone(),
            parent: parent.clone(),
        }))
        .await
        .unwrap();

    let parents_result = text_payload(
        &server
            .tool_list_card_parents(Parameters(ListCardParentsRequest {
                card: child.clone(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(parents_result["total"], 1);
    assert_eq!(parents_result["page"], 1);
    assert_eq!(parents_result["page_size"], 50);
    assert_eq!(
        parents_result["items"].as_array().unwrap()[0]["title"],
        "Parent"
    );

    let children_result = text_payload(
        &server
            .tool_list_card_children(Parameters(ListCardChildrenRequest {
                card: parent.clone(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(children_result["total"], 1);
    assert_eq!(children_result["page"], 1);
    assert_eq!(children_result["page_size"], 50);
    assert_eq!(
        children_result["items"].as_array().unwrap()[0]["title"],
        "Child"
    );
}

#[tokio::test]
async fn tool_remove_card_parent_returns_error_when_edge_missing() {
    use rmcp::model::ErrorCode;

    let (server, _tmp, parent, child) = setup_server_with_two_cards().await;

    let err = server
        .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
            child: child.clone(),
            parent: parent.clone(),
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    let msg = err.message.to_lowercase();
    assert!(
        msg.contains("not found") || msg.contains("missing") || msg.contains("does not exist"),
        "expected edge-not-found message, got: {msg}"
    );
}

#[tokio::test]
async fn tool_create_card_with_sprint_id_assigns_to_sprint() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    let sprint_result = server
        .tool_create_sprint(Parameters(sprint_req("B", "alpha")))
        .await
        .unwrap();
    let sprint_body = text_payload(&sprint_result);
    let sprint_id = sprint_body["id"].as_str().unwrap().to_string();

    let result = server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: Some(sprint_id.clone()),
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Sprinted".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    let body = text_payload(&result);
    assert_eq!(body["sprint_id"].as_str().unwrap(), sprint_id);
}

#[tokio::test]
async fn tool_create_card_with_sprint_name_resolves_and_assigns() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("B", "alpha")))
        .await
        .unwrap();
    let result = server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: Some("alpha".into()),
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Sprinted".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    let body = text_payload(&result);
    assert!(body["sprint_id"].is_string());
}

#[tokio::test]
async fn tool_create_card_without_sprint_id_leaves_card_unassigned() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    let result = server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: None,
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Plain".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();
    let body = text_payload(&result);
    assert!(body["sprint_id"].is_null());
}

/// Negative path: passing a sprint identifier the resolver can't find
/// surfaces a useful error mentioning both "Sprint" and the offending
/// name, not a panic or a generic message. Pins the contract that LLM
/// clients rely on when learning the tool schema by trial.
#[tokio::test]
async fn tool_create_card_with_unknown_sprint_name_returns_useful_error() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();

    let err = server
        .tool_create_card(Parameters(CreateCardParams {
            board: "B".into(),
            column: "TODO".into(),
            sprint: Some("nonexistent".into()),
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Sprinted".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("Sprint"), "err: {msg}");
    assert!(msg.contains("nonexistent"), "err: {msg}");
}

/// Negative path: a sprint UUID from another board cannot be used when
/// creating a card on the current board. The error returned by the tool
/// path comes from the typed `SprintBoardMismatch` variant via
/// `kanban_err_to_mcp`, so the message mentions "belongs to board".
#[tokio::test]
async fn tool_create_card_with_cross_board_sprint_returns_useful_error() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("A", Some("A".into()))))
        .await
        .unwrap();
    server
        .tool_create_board(Parameters(board_req("B", Some("B".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("A", "TODO")))
        .await
        .unwrap();
    let sprint_b_result = server
        .tool_create_sprint(Parameters(sprint_req("B", "beta")))
        .await
        .unwrap();
    let sprint_b_id = text_payload(&sprint_b_result)["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Pass sprint B's UUID while creating a card on board A. The
    // sprint resolver scoped to board A would reject this as a name
    // miss, so we pass the UUID directly to ensure we exercise the
    // domain-level cross-board check rather than the resolver miss.
    let err = server
        .tool_create_card(Parameters(CreateCardParams {
            board: "A".into(),
            column: "TODO".into(),
            sprint: Some(sprint_b_id.clone()),
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Sprinted".into(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("belongs to board"), "err: {msg}");
}

// KAN-792: the board-create tool funnels through the shared
// `kanban_service::api::v1::CreateBoardRequest` (the bespoke MCP create DTO is
// gone), converts via `into_new_board`, and calls `create_board_from_spec`.
#[tokio::test]
async fn test_mcp_create_board_uses_shared_dto() {
    let (server, _tmp) = setup_server().await;

    // The shared DTO carries the full create spec (not just name/card_prefix):
    // a client passing extra fields must have them applied through the factory.
    let req: CreateBoardRequest = serde_json::from_value(serde_json::json!({
        "name": "Roadmap",
        "card_prefix": "KAN",
        "sprint_prefix": "SPR",
        "description": "Q3",
        "sprint_duration_days": 21,
    }))
    .expect("shared CreateBoardRequest deserializes from MCP tool args");

    let result = server
        .tool_create_board(Parameters(req))
        .await
        .expect("create board tool succeeds");
    let body = text_payload(&result);

    assert_eq!(body["name"], "Roadmap");
    assert_eq!(body["card_prefix"], "KAN");
    assert_eq!(body["sprint_prefix"], "SPR");
    assert_eq!(body["description"], "Q3");
    assert_eq!(body["sprint_duration_days"], 21);
    // Server-managed factory output: a fresh board seeds position 0 and is
    // projected via BoardResponse (no internal counter fields leak).
    assert_eq!(body["position"], 0);
    assert!(
        body.get("card_counter").is_none(),
        "JSON edge must project via BoardResponse, not the domain Board: {body}"
    );
}

/// The shared DTO is the *only* board-create request type the MCP crate
/// re-exports: importing the formerly-bespoke `kanban_mcp::...board::CreateBoardRequest`
/// path resolves to the shared service type. Compile-asserts the duplication is gone.
#[test]
fn test_mcp_create_board_request_is_the_shared_service_type() {
    fn assert_same<T>(_: &T)
    where
        T: 'static,
    {
        assert_eq!(
            std::any::TypeId::of::<CreateBoardRequest>(),
            std::any::TypeId::of::<kanban_service::api::CreateBoardRequest>(),
            "MCP CreateBoardRequest must be the shared service DTO"
        );
    }
    let req = CreateBoardRequest {
        id: None,
        name: "x".into(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    };
    assert_same(&req);
}

// KAN-794: the column-create tool resolves the `board` name→id via the shared
// resolver and funnels through the Column factory (`create_column_from_spec`).
// The bespoke MCP create-content DTO is gone: the request's `content` field is
// the shared `kanban_service::api::CreateColumnRequest`.
#[tokio::test]
async fn test_mcp_create_column_uses_shared_factory() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("Roadmap", Some("KAN".into()))))
        .await
        .unwrap();

    // The content carries the shared create fields (here a client wip_limit);
    // the tool resolves "Roadmap" by name and creates via the factory.
    let req = CreateColumnParams {
        board: "Roadmap".into(),
        content: kanban_service::api::CreateColumnRequest {
            id: None,
            name: "In Review".into(),
            wip_limit: Some(4),
        },
    };
    let result = server
        .tool_create_column(Parameters(req))
        .await
        .expect("create column tool succeeds");
    let body = text_payload(&result);

    assert_eq!(body["name"], "In Review");
    assert_eq!(body["wip_limit"], 4);
    // Server-assigned append position (first column under a fresh board).
    assert_eq!(body["position"], 0);
    // JSON edge projects via ColumnResponse: the documented wire fields present.
    assert!(body.get("board_id").is_some());
    assert!(body.get("created_at").is_some());
}

/// The MCP column-create request flattens the shared service content DTO: its
/// `content` field is exactly `kanban_service::api::CreateColumnRequest`, so the
/// create fields are not re-derived. Compile-asserts the bespoke content is gone.
#[test]
fn test_mcp_create_column_content_is_the_shared_service_type() {
    fn assert_same<T: 'static>(_: &T) {
        assert_eq!(
            std::any::TypeId::of::<T>(),
            std::any::TypeId::of::<kanban_service::api::CreateColumnRequest>(),
            "MCP column-create content must be the shared service DTO"
        );
    }
    let req = CreateColumnParams {
        board: "B".into(),
        content: kanban_service::api::CreateColumnRequest {
            id: None,
            name: "x".into(),
            wip_limit: None,
        },
    };
    assert_same(&req.content);
}

// ============================================================================
// KAN-796: the card-create tool funnels through the shared
// `kanban_service::api::v1::CreateCardRequest` (the bespoke MCP create content
// is gone), resolves the `board`/`column`/`sprint` name-or-id references, splits
// the shared content via `into_new_card(column_id)`, and projects the resulting
// domain Card via `CardResponse`.
// ============================================================================

/// Minimal-path card-create request: the `board`/`column` names plus the shared
/// `kanban_service::api::CreateCardRequest` content (just a title here).
fn card_req(board: &str, column: &str, title: &str) -> CreateCardParams {
    CreateCardParams {
        board: board.to_string(),
        column: column.to_string(),
        sprint: None,
        content: kanban_service::api::CreateCardRequest {
            id: None,
            title: title.to_string(),
            description: None,
            priority: None,
            due_date: None,
            points: None,
            sprint_id: None,
        },
    }
}

#[tokio::test]
async fn test_mcp_create_card_uses_shared_dto_and_factory() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();

    // The content carries the shared create fields (here a client priority +
    // points); the tool resolves "B"/"TODO" by name and creates via the factory.
    let req = CreateCardParams {
        board: "B".into(),
        column: "TODO".into(),
        sprint: None,
        content: kanban_service::api::CreateCardRequest {
            id: None,
            title: "Funnelled".into(),
            description: Some("via shared DTO".into()),
            priority: Some(kanban_service::api::CardPriorityDto::High),
            due_date: None,
            points: Some(8),
            sprint_id: None,
        },
    };
    let result = server
        .tool_create_card(Parameters(req))
        .await
        .expect("create card tool succeeds");
    let body = text_payload(&result);

    assert_eq!(body["title"], "Funnelled");
    assert_eq!(body["description"], "via shared DTO");
    assert_eq!(body["points"], 8);
    // Factory-seeded user-facing number (first card on the board).
    assert_eq!(body["card_number"], 1);
    // JSON edge projects via CardResponse: wire enum is snake_case, internal
    // sprint_logs hidden.
    assert_eq!(body["priority"], "high");
    assert_eq!(body["status"], "todo");
    assert!(
        body.get("sprint_logs").is_none(),
        "CardResponse hides internal sprint_logs: {body}"
    );
}

/// The card-create content the tool funnels is the shared service DTO (the
/// bespoke create content is gone). Compile-asserts the content type identity.
#[test]
fn test_mcp_create_card_content_is_the_shared_service_type() {
    fn assert_same<T: 'static>(_: &T) {
        assert_eq!(
            std::any::TypeId::of::<T>(),
            std::any::TypeId::of::<kanban_service::api::CreateCardRequest>(),
            "MCP card-create content must be the shared service DTO"
        );
    }
    let req = card_req("B", "TODO", "x");
    assert_same(&req.content);
}

#[tokio::test]
async fn test_mcp_create_card_resolves_sprint_name_through_shared_funnel() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    let sprint_result = server
        .tool_create_sprint(Parameters(sprint_req("B", "alpha")))
        .await
        .unwrap();
    let sprint_id = text_payload(&sprint_result)["id"]
        .as_str()
        .unwrap()
        .to_string();

    // The loose `sprint` name resolves to the sprint id before `into_new_card`.
    let mut req = card_req("B", "TODO", "Sprinted");
    req.sprint = Some("alpha".into());
    let result = server.tool_create_card(Parameters(req)).await.unwrap();
    let body = text_payload(&result);
    assert_eq!(body["sprint_id"].as_str().unwrap(), sprint_id);
}

/// KAN-798: `tool_create_sprint` consumes the SHARED
/// `kanban_service::api::CreateSprintRequest` content (flattened under the MCP
/// board name-or-id) and funnels it through the Sprint factory via
/// `create_sprint_from_spec` — minting the user-facing `sprint_number` from the
/// board counter and projecting the result via `SprintResponse` (snake_case
/// status, resolved `name`, hidden `name_index`). The bespoke MCP content DTO is
/// gone; the request literal below only compiles against the shared content.
#[tokio::test]
async fn test_mcp_create_sprint_uses_shared_dto_and_factory() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();

    // The content is the shared create DTO (id/name/prefix/card_prefix). The
    // tool resolves "B" by name and creates via the factory.
    let req = CreateSprintParams {
        board: "B".into(),
        content: kanban_service::api::CreateSprintRequest {
            id: None,
            name: Some("Alpha".into()),
            prefix: Some("SPR".into()),
            card_prefix: None,
        },
    };
    let result = server
        .tool_create_sprint(Parameters(req))
        .await
        .expect("create sprint tool succeeds");
    let body = text_payload(&result);

    // Factory-seeded user-facing number (first sprint on the board).
    assert_eq!(body["sprint_number"], 1);
    assert_eq!(body["prefix"], "SPR");
    // SprintResponse projection: resolved name, snake_case status, no
    // internal allocation state leaked.
    assert_eq!(body["name"], "Alpha");
    assert_eq!(body["status"], "planning");
    assert!(
        body.get("name_index").is_none(),
        "SprintResponse hides internal name_index: {body}"
    );
}

/// Regression guard for KAN-769: every MCP read tool must project its result
/// through the v1 Response DTO, exactly like create/get-board already did.
/// Before this fix, `list_boards`/`get_board`/`list_columns`/`get_column`/
/// `list_sprints`/`get_sprint`/`get_card`/update tools serialized the raw domain
/// entity, leaking internal bookkeeping (`card_counter`, sprint counters /
/// name-pool indices, `sprint_logs`, sprint `name_index`). This drives each read
/// tool end-to-end and asserts none of those internal fields appear on the wire,
/// while the documented DTO fields are present.
#[tokio::test]
async fn read_tools_project_through_v1_response_dtos_hiding_internal_state() {
    let (server, _tmp) = setup_server().await;

    // Seed a board → column → sprint → card through the create tools.
    server
        .tool_create_board(Parameters(board_req("Roadmap", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("Roadmap", "To Do")))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("Roadmap", "Alpha")))
        .await
        .unwrap();
    let card_body = text_payload(
        &server
            .tool_create_card(Parameters(CreateCardParams {
                board: "Roadmap".into(),
                column: "To Do".into(),
                sprint: None,
                content: kanban_service::api::CreateCardRequest {
                    id: None,
                    title: "Ship it".into(),
                    description: None,
                    priority: None,
                    due_date: None,
                    points: None,
                    sprint_id: None,
                },
            }))
            .await
            .expect("create card tool succeeds"),
    );
    let card_id = card_body["id"].as_str().unwrap().to_string();

    let board_hidden = ["card_counter", "next_sprint_number", "sprint_counters"];
    let card_hidden = ["sprint_logs"];

    // list_boards: paginated envelope — no internal counters in the items.
    let boards = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    let board0 = &mcp_list_boards_items(&boards)[0];
    assert_eq!(board0["name"], "Roadmap");
    for f in board_hidden {
        assert!(board0.get(f).is_none(), "list_boards leaked {f}: {boards}");
    }

    // get_board: BoardResponse.
    let board = text_payload(
        &server
            .tool_get_board(Parameters(GetBoardRequest {
                board: "Roadmap".into(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(board["name"], "Roadmap");
    for f in board_hidden {
        assert!(board.get(f).is_none(), "get_board leaked {f}: {board}");
    }

    // list_columns + get_column: ColumnResponse(s), paginated envelope.
    let cols = text_payload(
        &server
            .tool_list_columns(Parameters(ListColumnsRequest {
                board: "Roadmap".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    let col0 = &cols["items"].as_array().expect("list_columns items")[0];
    assert_eq!(col0["name"], "To Do");
    assert!(col0.get("board_id").is_some());
    let col = text_payload(
        &server
            .tool_get_column(Parameters(GetColumnRequest {
                column: "To Do".into(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(col["name"], "To Do");

    // list_sprints + get_sprint: SprintResponse(s), paginated envelope — resolved
    // name, no name_index.
    let sprints = text_payload(
        &server
            .tool_list_sprints(Parameters(ListSprintsRequest {
                board: "Roadmap".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    let spr0 = &sprints["items"].as_array().expect("list_sprints items")[0];
    assert_eq!(spr0["name"], "Alpha");
    assert_eq!(spr0["sprint_number"], 1);
    assert!(
        spr0.get("name_index").is_none(),
        "list_sprints leaked name_index: {sprints}"
    );
    let sprint = text_payload(
        &server
            .tool_get_sprint(Parameters(GetSprintRequest {
                sprint: "Alpha".into(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(sprint["name"], "Alpha");
    assert!(
        sprint.get("name_index").is_none(),
        "get_sprint leaked name_index: {sprint}"
    );

    // get_card: CardResponse — exposes card_number, hides sprint_logs.
    let card = text_payload(
        &server
            .tool_get_card(Parameters(GetCardRequest {
                card: card_id.clone(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(card["title"], "Ship it");
    assert_eq!(card["card_number"], 1);
    for f in card_hidden {
        assert!(card.get(f).is_none(), "get_card leaked {f}: {card}");
    }
}

#[tokio::test]
async fn test_mcp_list_columns_returns_paginated_envelope() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "TODO")))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("B", "Doing")))
        .await
        .unwrap();

    let result = text_payload(
        &server
            .tool_list_columns(Parameters(ListColumnsRequest {
                board: "B".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );

    assert_eq!(result["total"], 2, "envelope must report total: {result}");
    assert_eq!(result["page"], 1);
    assert_eq!(result["page_size"], 50);
    let items = result["items"]
        .as_array()
        .expect("envelope must carry items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "TODO");
}

#[tokio::test]
async fn test_mcp_list_sprints_returns_paginated_envelope() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("B", "Alpha")))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(sprint_req("B", "Beta")))
        .await
        .unwrap();

    let result = text_payload(
        &server
            .tool_list_sprints(Parameters(ListSprintsRequest {
                board: "B".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );

    assert_eq!(result["total"], 2, "envelope must report total: {result}");
    assert_eq!(result["page"], 1);
    assert_eq!(result["page_size"], 50);
    let items = result["items"]
        .as_array()
        .expect("envelope must carry items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "Alpha");
}

#[tokio::test]
async fn test_mcp_list_archived_cards_includes_board_id() {
    let (server, _tmp) = setup_server().await;
    let board = text_payload(
        &server
            .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
            .await
            .unwrap(),
    );
    let board_id = board["id"].as_str().unwrap().to_string();

    server
        .tool_create_column(Parameters(column_req("Alpha", "TODO")))
        .await
        .unwrap();
    let created = text_payload(
        &server
            .tool_create_card(Parameters(CreateCardParams {
                board: "Alpha".into(),
                column: "TODO".into(),
                sprint: None,
                content: kanban_service::api::CreateCardRequest {
                    id: None,
                    title: "the card".into(),
                    description: Some("desc".into()),
                    priority: None,
                    due_date: None,
                    points: None,
                    sprint_id: None,
                },
            }))
            .await
            .unwrap(),
    );
    let card_id = created["id"].as_str().unwrap().to_string();
    server
        .tool_archive_card(Parameters(kanban_mcp::ArchiveCardRequest { card: card_id }))
        .await
        .unwrap();

    let listed = text_payload(
        &server
            .tool_list_cards(Parameters(kanban_mcp::ListCardsRequest {
                board: None,
                column: None,
                sprint: None,
                status: None,
                archived: Some("only".into()),
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    let item = &listed["items"][0];
    // list_cards with archived='only' returns the lean CardSummary plus a
    // top-level `archived_at` — no nested `card`, no restore-context, and (like
    // the live list) no `description`.
    assert!(item["archived_at"].is_string());
    assert_eq!(item["title"], "the card");
    let obj = item.as_object().unwrap();
    assert!(obj.get("card").is_none(), "no nested card object");
    assert!(
        obj.get("original_column_id").is_none(),
        "no original_column_id"
    );
    assert!(obj.get("board_id").is_none(), "no first-class board_id");
    let _ = board_id;
}

#[tokio::test]
async fn test_mcp_list_cards_archived_selector() {
    // I2 (KAN-882): the unified list_cards tool with the three-state `archived`
    // selector replaces the separate archived tool.
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("Alpha", "TODO")))
        .await
        .unwrap();
    let mk_card = |title: &str| CreateCardParams {
        board: "Alpha".into(),
        column: "TODO".into(),
        sprint: None,
        content: kanban_service::api::CreateCardRequest {
            id: None,
            title: title.into(),
            description: None,
            priority: None,
            due_date: None,
            points: None,
            sprint_id: None,
        },
    };
    server
        .tool_create_card(Parameters(mk_card("Live")))
        .await
        .unwrap();
    let archived_id = text_payload(
        &server
            .tool_create_card(Parameters(mk_card("Archived")))
            .await
            .unwrap(),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    server
        .tool_archive_card(Parameters(kanban_mcp::ArchiveCardRequest {
            card: archived_id,
        }))
        .await
        .unwrap();

    let req = |archived: Option<&str>| kanban_mcp::ListCardsRequest {
        board: None,
        column: None,
        sprint: None,
        status: None,
        archived: archived.map(|s| s.to_string()),
        sort: None,
        order: None,
        page: None,
        page_size: None,
    };

    // default / exclude: live only, no archived_at.
    let live = text_payload(&server.tool_list_cards(Parameters(req(None))).await.unwrap());
    assert_eq!(live["total"], 1);
    assert_eq!(live["items"][0]["title"], "Live");
    assert!(live["items"][0].get("archived_at").is_none());

    // only: archived, stamped.
    let only = text_payload(
        &server
            .tool_list_cards(Parameters(req(Some("only"))))
            .await
            .unwrap(),
    );
    assert_eq!(only["total"], 1);
    assert_eq!(only["items"][0]["title"], "Archived");
    assert!(only["items"][0]["archived_at"].is_string());

    // include: both.
    let both = text_payload(
        &server
            .tool_list_cards(Parameters(req(Some("include"))))
            .await
            .unwrap(),
    );
    assert_eq!(both["total"], 2);

    // invalid selector is rejected.
    assert!(server
        .tool_list_cards(Parameters(req(Some("bogus"))))
        .await
        .is_err());
}

// ---- I5 (KAN-887): the MCP board surface — archived selector + subcommands ----

fn list_boards_req(archived: Option<&str>) -> ListBoardsRequest {
    ListBoardsRequest {
        archived: archived.map(|s| s.to_string()),
        sort: None,
        order: None,
        page: None,
        page_size: None,
    }
}

async fn mcp_list_boards(server: &KanbanMcpServer, archived: Option<&str>) -> Value {
    text_payload(
        &server
            .tool_list_boards(Parameters(list_boards_req(archived)))
            .await
            .unwrap(),
    )
}

fn mcp_list_boards_count(val: &Value) -> usize {
    val["total"].as_u64().unwrap_or(0) as usize
}

fn mcp_list_boards_items(val: &Value) -> &Vec<Value> {
    val["items"]
        .as_array()
        .expect("list_boards always returns the paginated envelope")
}

/// Create a board via the tool and return its id.
async fn mcp_create_board(server: &KanbanMcpServer, name: &str) -> String {
    text_payload(
        &server
            .tool_create_board(Parameters(board_req(name, Some("B".into()))))
            .await
            .unwrap(),
    )["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_mcp_list_boards_archived_selector() {
    let (server, _tmp) = setup_server().await;
    let _live = mcp_create_board(&server, "Live Board").await;
    let archived = mcp_create_board(&server, "Archived Board").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest {
            board: archived.clone(),
        }))
        .await
        .unwrap();

    // default / exclude: live only, no archived_at.
    let live = mcp_list_boards(&server, None).await;
    assert_eq!(mcp_list_boards_count(&live), 1);
    let live_arr = mcp_list_boards_items(&live);
    assert_eq!(live_arr[0]["name"], "Live Board");
    assert!(live_arr[0].get("archived_at").is_none());

    let excluded = mcp_list_boards(&server, Some("exclude")).await;
    assert_eq!(mcp_list_boards_count(&excluded), 1);

    // only: archived, stamped with archived_at.
    let only = mcp_list_boards(&server, Some("only")).await;
    assert_eq!(mcp_list_boards_count(&only), 1);
    let only_arr = mcp_list_boards_items(&only);
    assert_eq!(only_arr[0]["name"], "Archived Board");
    assert!(only_arr[0]["archived_at"].is_string());

    // include: both, one stamped and one not.
    let both = mcp_list_boards(&server, Some("include")).await;
    assert_eq!(mcp_list_boards_count(&both), 2);
    let stamped: Vec<bool> = mcp_list_boards_items(&both)
        .iter()
        .map(|i| i.get("archived_at").is_some())
        .collect();
    assert!(stamped.contains(&true) && stamped.contains(&false));

    // invalid selector is rejected.
    assert!(server
        .tool_list_boards(Parameters(list_boards_req(Some("bogus"))))
        .await
        .is_err());
}

// B5a (KAN-929): the board list tool consumes the service filter
// (`list_boards_filtered`) as the single gather path, projecting via
// `BoardResponse`. These pin the three selector states plus the live-shape
// guard directly on the tool.

#[tokio::test]
async fn test_mcp_list_boards_default_live() {
    let (server, _tmp) = setup_server().await;
    let _live = mcp_create_board(&server, "Live Board").await;
    let archived = mcp_create_board(&server, "Archived Board").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest {
            board: archived.clone(),
        }))
        .await
        .unwrap();

    // Default (no selector): live boards only, and the live shape carries no
    // archived_at key.
    let live = mcp_list_boards(&server, None).await;
    assert_eq!(mcp_list_boards_count(&live), 1);
    let arr = mcp_list_boards_items(&live);
    assert_eq!(arr[0]["name"], "Live Board");
    assert!(
        arr[0].get("archived_at").is_none(),
        "a live board must not carry an archived_at key: {live}"
    );

    // Explicit `exclude` matches the default.
    let excluded = mcp_list_boards(&server, Some("exclude")).await;
    assert_eq!(mcp_list_boards_count(&excluded), 1);
    assert!(mcp_list_boards_items(&excluded)[0]
        .get("archived_at")
        .is_none());
}

#[tokio::test]
async fn test_mcp_list_boards_archived_only() {
    let (server, _tmp) = setup_server().await;
    let _live = mcp_create_board(&server, "Live Board").await;
    let archived = mcp_create_board(&server, "Archived Board").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest {
            board: archived.clone(),
        }))
        .await
        .unwrap();

    let only = mcp_list_boards(&server, Some("only")).await;
    assert_eq!(mcp_list_boards_count(&only), 1);
    let arr = mcp_list_boards_items(&only);
    assert_eq!(arr[0]["name"], "Archived Board");
    assert!(
        arr[0]["archived_at"].is_string(),
        "an archived board must be stamped with archived_at: {only}"
    );
}

#[tokio::test]
async fn test_mcp_list_boards_include_both() {
    let (server, _tmp) = setup_server().await;
    let _live = mcp_create_board(&server, "Live Board").await;
    let archived = mcp_create_board(&server, "Archived Board").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest {
            board: archived.clone(),
        }))
        .await
        .unwrap();

    let both = mcp_list_boards(&server, Some("include")).await;
    assert_eq!(mcp_list_boards_count(&both), 2);
    let arr = mcp_list_boards_items(&both);
    // Exactly one live (no archived_at) and one archived (stamped).
    let live = arr
        .iter()
        .find(|b| b["name"] == "Live Board")
        .expect("live board present");
    let arch = arr
        .iter()
        .find(|b| b["name"] == "Archived Board")
        .expect("archived board present");
    assert!(
        live.get("archived_at").is_none(),
        "the live board in the combined list must not carry archived_at: {both}"
    );
    assert!(
        arch["archived_at"].is_string(),
        "the archived board in the combined list must be stamped: {both}"
    );
}

#[tokio::test]
async fn test_mcp_archive_and_restore_board() {
    let (server, _tmp) = setup_server().await;
    let id = mcp_create_board(&server, "Round Trip").await;

    // Archive: leaves the live list.
    let archived = text_payload(
        &server
            .tool_archive_board(Parameters(ArchiveBoardRequest { board: id.clone() }))
            .await
            .unwrap(),
    );
    assert_eq!(archived["archived"].as_str().unwrap(), id);
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        0
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        1
    );

    // Restore (by UUID): returns to the live list, projected via BoardResponse.
    let restored = text_payload(
        &server
            .tool_restore_board(Parameters(RestoreBoardRequest { board: id.clone() }))
            .await
            .unwrap(),
    );
    assert_eq!(restored["id"].as_str().unwrap(), id);
    assert!(
        restored.get("archived_at").is_none(),
        "restored board is live: no archived_at"
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        1
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
}

#[tokio::test]
async fn test_mcp_restore_board_by_archived_name() {
    // An archived board is not in the live list, so name resolution must fall
    // back to the archived view.
    let (server, _tmp) = setup_server().await;
    let id = mcp_create_board(&server, "By Name").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest { board: id }))
        .await
        .unwrap();

    server
        .tool_restore_board(Parameters(RestoreBoardRequest {
            board: "By Name".into(),
        }))
        .await
        .unwrap();
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        1
    );
}

#[tokio::test]
async fn test_mcp_delete_archived_board_permanent() {
    let (server, _tmp) = setup_server().await;
    let id = mcp_create_board(&server, "Doomed").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest { board: id.clone() }))
        .await
        .unwrap();
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        1
    );

    let deleted = text_payload(
        &server
            .tool_delete_archived_board(Parameters(DeleteArchivedBoardRequest {
                board: id.clone(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(deleted["deleted"].as_str().unwrap(), id);

    // Absent from BOTH the live and archived lists afterward.
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        0
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("include")).await),
        0
    );
}

// REGR-4 (KAN-894): archived-scoped MCP tools resolve ONLY archived boards, so a
// same-named live board can never be hit.
#[tokio::test]
async fn test_mcp_delete_archived_board_name_collision_targets_archived() {
    let (server, _tmp) = setup_server().await;
    let live = mcp_create_board(&server, "Roadmap").await;
    let arch = mcp_create_board(&server, "Roadmap").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest { board: arch }))
        .await
        .unwrap();
    server
        .tool_delete_archived_board(Parameters(DeleteArchivedBoardRequest {
            board: "Roadmap".into(),
        }))
        .await
        .unwrap();
    let live_list = mcp_list_boards(&server, None).await;
    let live_arr = mcp_list_boards_items(&live_list);
    assert_eq!(live_arr.len(), 1);
    assert_eq!(live_arr[0]["id"].as_str().unwrap(), live);
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
}

#[tokio::test]
async fn test_mcp_restore_board_name_collision_targets_archived() {
    let (server, _tmp) = setup_server().await;
    let _live = mcp_create_board(&server, "Roadmap").await;
    let arch = mcp_create_board(&server, "Roadmap").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest { board: arch }))
        .await
        .unwrap();
    server
        .tool_restore_board(Parameters(RestoreBoardRequest {
            board: "Roadmap".into(),
        }))
        .await
        .unwrap();
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        2
    );
}

// B5b (KAN-930): archived-board name resolution runs through the
// `list_boards_filtered(ArchivedOnly)` filter path (dropping the bespoke
// `mcp_resolve_archived_board`). The candidate set is archived-only, so the
// KAN-894 guard (never touch a live board) is structural, and a UUID still
// passes straight through.

#[tokio::test]
async fn test_mcp_restore_board_by_name_targets_archived_not_live() {
    // Two boards share the SAME name: one live, one archived. Restore-by-name
    // must resolve the ARCHIVED one (KAN-894). If the resolver drew from the
    // live/full set, it would either hit the live board or report ambiguity.
    let (server, _tmp) = setup_server().await;
    let live = mcp_create_board(&server, "Roadmap").await;
    let arch = mcp_create_board(&server, "Roadmap").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest {
            board: arch.clone(),
        }))
        .await
        .unwrap();

    let restored = text_payload(
        &server
            .tool_restore_board(Parameters(RestoreBoardRequest {
                board: "Roadmap".into(),
            }))
            .await
            .unwrap(),
    );
    // The archived board is the one returned to live, not the pre-existing live one.
    assert_eq!(
        restored["id"].as_str().unwrap(),
        arch,
        "restore-by-name must target the archived board, not the same-named live one"
    );
    // Both are now live; none remain archived.
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        2
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
    // The live board was never disturbed.
    let live_now = mcp_list_boards(&server, None).await;
    assert!(mcp_list_boards_items(&live_now)
        .iter()
        .any(|b| b["id"].as_str().unwrap() == live));
}

#[tokio::test]
async fn test_mcp_delete_archived_by_name_resolves_archived() {
    // Same-named live + archived boards. Permanent delete-by-name must resolve
    // and remove ONLY the archived board (KAN-894), leaving the live one intact.
    let (server, _tmp) = setup_server().await;
    let live = mcp_create_board(&server, "Roadmap").await;
    let arch = mcp_create_board(&server, "Roadmap").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest {
            board: arch.clone(),
        }))
        .await
        .unwrap();

    let deleted = text_payload(
        &server
            .tool_delete_archived_board(Parameters(DeleteArchivedBoardRequest {
                board: "Roadmap".into(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(
        deleted["deleted"].as_str().unwrap(),
        arch,
        "delete-by-name must target the archived board"
    );
    // The archived collection is now empty; the live board survives, untouched.
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
    let live_list = mcp_list_boards(&server, None).await;
    let live_arr = mcp_list_boards_items(&live_list);
    assert_eq!(live_arr.len(), 1);
    assert_eq!(live_arr[0]["id"].as_str().unwrap(), live);
}

#[tokio::test]
async fn test_mcp_restore_board_by_uuid_still_resolves_archived() {
    // UUID passthrough: an archived board restored by its raw UUID skips name
    // resolution entirely.
    let (server, _tmp) = setup_server().await;
    let id = mcp_create_board(&server, "By UUID").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest { board: id.clone() }))
        .await
        .unwrap();

    let restored = text_payload(
        &server
            .tool_restore_board(Parameters(RestoreBoardRequest { board: id.clone() }))
            .await
            .unwrap(),
    );
    assert_eq!(restored["id"].as_str().unwrap(), id);
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        1
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
}

// KAN-905: archived-board name resolution uses case-insensitive matching.

#[tokio::test]
async fn test_mcp_restore_archived_board_by_case_insensitive_name_resolves() {
    let (server, _tmp) = setup_server().await;
    let id = mcp_create_board(&server, "Roadmap 2026").await;
    server
        .tool_archive_board(Parameters(ArchiveBoardRequest { board: id }))
        .await
        .unwrap();
    // Restore by lowercase name — case-insensitive match.
    let restored = text_payload(
        &server
            .tool_restore_board(Parameters(RestoreBoardRequest {
                board: "roadmap 2026".into(),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(restored["name"], "Roadmap 2026");
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, None).await),
        1
    );
    assert_eq!(
        mcp_list_boards_count(&mcp_list_boards(&server, Some("only")).await),
        0
    );
}

#[tokio::test]
async fn test_list_boards_default_page_is_discoverably_truncated_via_total() {
    let (server, _tmp) = setup_server().await;
    for i in 0..60 {
        mcp_create_board(&server, &format!("Board {i}")).await;
    }
    // No page/page_size: the first default-sized page, not everything — but
    // `total` makes the truncation discoverable so a caller can page further.
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(result["total"], 60, "total reflects all 60 boards");
    assert_eq!(
        result["items"].as_array().unwrap().len(),
        50,
        "the default page_size caps the first page at 50"
    );
}

#[tokio::test]
async fn test_list_boards_explicit_pagination_still_works() {
    let (server, _tmp) = setup_server().await;
    for i in 0..20 {
        mcp_create_board(&server, &format!("Board {i}")).await;
    }
    // Explicit page/page_size — returns paginated wrapper.
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: None,
                order: None,
                page: Some(1),
                page_size: Some(10),
            }))
            .await
            .unwrap(),
    );
    assert_eq!(result["total"], 20, "total reflects all boards");
    assert_eq!(
        result["items"].as_array().unwrap().len(),
        10,
        "page_size=10 yields 10 items"
    );
}

#[tokio::test]
async fn test_list_boards_include_archived_total_reflects_all_pages() {
    let (server, _tmp) = setup_server().await;
    for i in 0..55 {
        mcp_create_board(&server, &format!("Live {i}")).await;
    }
    for i in 0..10 {
        let id = mcp_create_board(&server, &format!("Archived {i}")).await;
        server
            .tool_archive_board(Parameters(ArchiveBoardRequest { board: id }))
            .await
            .unwrap();
    }
    // include archived — total reflects all 65, first page capped at 50.
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: Some("include".into()),
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(result["total"], 65, "55 live + 10 archived = 65 total");
    assert_eq!(result["items"].as_array().unwrap().len(), 50);
}

// KAN-902: list_cards with archived='only' returns the lean CardSummary shape
// (not the old ArchivedCardResponse).

#[tokio::test]
async fn test_list_archived_cards_returns_card_summary_shape() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("Alpha", "TODO")))
        .await
        .unwrap();
    let created = text_payload(
        &server
            .tool_create_card(Parameters(CreateCardParams {
                board: "Alpha".into(),
                column: "TODO".into(),
                sprint: None,
                content: kanban_service::api::CreateCardRequest {
                    id: None,
                    title: "shape check".into(),
                    description: Some("detailed desc".into()),
                    priority: None,
                    due_date: None,
                    points: None,
                    sprint_id: None,
                },
            }))
            .await
            .unwrap(),
    );
    let card_id = created["id"].as_str().unwrap().to_string();
    server
        .tool_archive_card(Parameters(kanban_mcp::ArchiveCardRequest { card: card_id }))
        .await
        .unwrap();

    let listed = text_payload(
        &server
            .tool_list_cards(Parameters(kanban_mcp::ListCardsRequest {
                board: None,
                column: None,
                sprint: None,
                status: None,
                archived: Some("only".into()),
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    let item = &listed["items"][0];
    // CardSummary shape: has archived_at and title.
    assert!(
        item["archived_at"].is_string(),
        "archived_at must be present"
    );
    assert_eq!(item["title"], "shape check");
    // Old ArchivedCardResponse fields must be absent.
    let obj = item.as_object().unwrap();
    assert!(
        obj.get("description").is_none(),
        "description must not be present (CardSummary has no description)"
    );
    assert!(obj.get("card").is_none(), "no nested card object");
    assert!(
        obj.get("original_column_id").is_none(),
        "no original_column_id"
    );
    assert!(obj.get("board_id").is_none(), "no first-class board_id");
}

// KAN-947: list_boards honors sort/order params, and set_board_sort persists
// the AppConfig board-sort default so subsequent unsorted list calls reflect it.

/// Read the ordered board names from a list_boards paginated envelope.
fn board_names(val: &Value) -> Vec<String> {
    mcp_list_boards_items(val)
        .iter()
        .map(|b| b["name"].as_str().unwrap().to_string())
        .collect()
}

/// Seed three boards in an order that makes position and name orderings differ.
/// Positions are assigned on create (Charlie=0, Alpha=1, Bravo=2), so the
/// built-in Position-ASC default yields [Charlie, Alpha, Bravo], while Name-ASC
/// yields [Alpha, Bravo, Charlie].
async fn seed_three_boards(server: &KanbanMcpServer) {
    for name in ["Charlie", "Alpha", "Bravo"] {
        mcp_create_board(server, name).await;
    }
}

#[tokio::test]
async fn test_mcp_list_boards_always_returns_paginated_envelope() {
    let (server, _tmp) = setup_server().await;
    seed_three_boards(&server).await;

    // No page/page_size supplied: the response must still be the same
    // PaginatedList envelope list_cards always returns, not a bare array.
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );

    assert_eq!(result["total"], 3, "envelope must report total: {result}");
    assert_eq!(result["page"], 1, "envelope must report page: {result}");
    assert_eq!(
        result["page_size"], 50,
        "envelope must report page_size: {result}"
    );
    assert_eq!(
        result["items"].as_array().map(|a| a.len()),
        Some(3),
        "envelope must carry items array: {result}"
    );
}

#[tokio::test]
async fn test_mcp_list_boards_sort_by_name() {
    let (server, _tmp) = setup_server().await;
    seed_three_boards(&server).await;
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: Some("name".into()),
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(
        board_names(&result),
        vec!["Alpha", "Bravo", "Charlie"],
        "sort=name must order boards alphabetically ascending"
    );
}

#[tokio::test]
async fn test_mcp_list_boards_order_desc() {
    let (server, _tmp) = setup_server().await;
    seed_three_boards(&server).await;
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: Some("name".into()),
                order: Some("desc".into()),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(
        board_names(&result),
        vec!["Charlie", "Bravo", "Alpha"],
        "order=desc must reverse the name ordering"
    );
}

#[tokio::test]
async fn test_mcp_set_board_sort_persists_default() {
    let dir = TempDir::new().unwrap();
    let data_path = dir.path().join("test.json");
    let config_path = dir.path().join("config.toml");
    let store_manager = default_store_manager();
    // Point the config at a temp file so config::save writes there, not the
    // global user config location.
    let config = AppConfig {
        configuration_location: Some(config_path.to_string_lossy().to_string()),
        ..AppConfig::default()
    };
    let server = KanbanMcpServer::new(&store_manager, &data_path.to_string_lossy(), config)
        .await
        .unwrap();
    seed_three_boards(&server).await;

    // Set the default board sort to Name-ASC.
    server
        .tool_set_board_sort(Parameters(kanban_mcp::SetBoardSortRequest {
            sort: Some("name".into()),
            order: Some("asc".into()),
        }))
        .await
        .unwrap();

    // An unsorted list must now reflect the configured default.
    let result = text_payload(
        &server
            .tool_list_boards(Parameters(ListBoardsRequest {
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap(),
    );
    assert_eq!(
        board_names(&result),
        vec!["Alpha", "Bravo", "Charlie"],
        "after set_board_sort, unsorted list must use the Name-ASC default"
    );

    // And the default must be persisted to the config file on disk.
    let persisted = std::fs::read_to_string(&config_path).expect("config file must be written");
    assert!(
        persisted.contains("board_sort_field"),
        "persisted config must carry board_sort_field, got: {persisted}"
    );
}

#[tokio::test]
async fn test_mcp_set_board_sort_echoes_resolved_order_not_null() {
    let dir = TempDir::new().unwrap();
    let data_path = dir.path().join("test.json");
    let config_path = dir.path().join("config.toml");
    let store_manager = default_store_manager();
    let config = AppConfig {
        configuration_location: Some(config_path.to_string_lossy().to_string()),
        ..AppConfig::default()
    };
    let server = KanbanMcpServer::new(&store_manager, &data_path.to_string_lossy(), config)
        .await
        .unwrap();

    // Only `sort` is supplied; `order` is omitted and must be resolved (to the
    // live default, Ascending, since no prior sort was persisted) rather than
    // echoed back as the raw `null` the caller sent.
    let response = text_payload(
        &server
            .tool_set_board_sort(Parameters(kanban_mcp::SetBoardSortRequest {
                sort: Some("name".into()),
                order: None,
            }))
            .await
            .unwrap(),
    );

    assert_eq!(
        response["board_sort_field"], "name",
        "resolved field must be echoed"
    );
    assert_eq!(
        response["board_sort_order"], "ascending",
        "omitted order must be echoed as the resolved+persisted value, not null: {response}"
    );
}

// KAN-954 (BSF-R5): the MCP board-sort setter now routes through R3's
// persist-first, in-place `KanbanContext::set_board_sort` instead of rebuilding
// the context via `open_deferred`. The rebuild minted a fresh session id and
// discarded the per-session undo history; the helper leaves both intact. This
// exercises `McpContext::set_board_sort` directly (not the locked tool wrapper,
// which reloads on every write) so the session/undo invariants are observable.
#[tokio::test]
async fn test_mcp_set_board_sort_persists_and_preserves_session() {
    use std::str::FromStr;

    let dir = TempDir::new().unwrap();
    let data_path = dir.path().join("test.json");
    let config_path = dir.path().join("config.toml");
    let store_manager = default_store_manager();
    let config = AppConfig {
        configuration_location: Some(config_path.to_string_lossy().to_string()),
        ..AppConfig::default()
    };
    let mut ctx = McpContext::new(&store_manager, &data_path.to_string_lossy(), config)
        .await
        .unwrap();

    // A mutation populates the per-session undo stack.
    ctx.create_board("Board".into(), None).unwrap();
    assert!(
        ctx.can_undo(),
        "a create must leave an undoable entry on the session stack"
    );
    let session_before = ctx.session_id();

    // Change the default board sort mid-session.
    ctx.set_board_sort(
        Some(kanban_domain::BoardSortField::from_str("name").unwrap()),
        Some(kanban_domain::SortOrder::from_str("asc").unwrap()),
    )
    .unwrap();

    // The session id is stable: no `open_deferred` rebuild happened.
    assert_eq!(
        ctx.session_id(),
        session_before,
        "set_board_sort must not mint a new session id (no context rebuild)"
    );
    // The undo history from earlier in the session survives.
    assert!(
        ctx.can_undo(),
        "set_board_sort must preserve the per-session undo history"
    );
    // And the preference is flushed to the config file on disk.
    let persisted = std::fs::read_to_string(&config_path).expect("config file must be written");
    assert!(
        persisted.contains("board_sort_field"),
        "persisted config must carry board_sort_field, got: {persisted}"
    );
    assert!(
        persisted.contains("board_sort_order"),
        "persisted config must carry board_sort_order, got: {persisted}"
    );
}

// KAN-962: single-entity get must stamp the archival marker's `archived_at` so
// an archived card/board is never returned looking live (get and list agree).

#[tokio::test]
async fn tool_get_card_stamps_archived_at_for_archived_card() {
    let (server, _tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
        .await
        .unwrap();
    server
        .tool_create_column(Parameters(column_req("Alpha", "TODO")))
        .await
        .unwrap();
    let created = text_payload(
        &server
            .tool_create_card(Parameters(CreateCardParams {
                board: "Alpha".into(),
                column: "TODO".into(),
                sprint: None,
                content: kanban_service::api::CreateCardRequest {
                    id: None,
                    title: "arch me".into(),
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
    let card_id = created["id"].as_str().unwrap().to_string();

    // Live: no archived_at key.
    let live = text_payload(
        &server
            .tool_get_card(Parameters(GetCardRequest {
                card: card_id.clone(),
            }))
            .await
            .unwrap(),
    );
    assert!(
        live.get("archived_at").is_none(),
        "live card get must not carry archived_at: {live}"
    );

    server
        .tool_archive_card(Parameters(kanban_mcp::ArchiveCardRequest {
            card: card_id.clone(),
        }))
        .await
        .unwrap();

    // Archived: get_card stamps the marker's archived_at.
    let archived = text_payload(
        &server
            .tool_get_card(Parameters(GetCardRequest { card: card_id }))
            .await
            .unwrap(),
    );
    assert!(
        archived.get("archived_at").is_some_and(|v| !v.is_null()),
        "archived card get must stamp archived_at: {archived}"
    );
}

#[tokio::test]
async fn tool_get_board_stamps_archived_at_for_archived_board() {
    let (server, _tmp) = setup_server().await;
    let board = text_payload(
        &server
            .tool_create_board(Parameters(board_req("Alpha", Some("A".into()))))
            .await
            .unwrap(),
    );
    let board_id = board["id"].as_str().unwrap().to_string();

    server
        .tool_archive_board(Parameters(kanban_mcp::ArchiveBoardRequest {
            board: board_id.clone(),
        }))
        .await
        .unwrap();

    let archived = text_payload(
        &server
            .tool_get_board(Parameters(GetBoardRequest { board: board_id }))
            .await
            .unwrap(),
    );
    assert!(
        archived.get("archived_at").is_some_and(|v| !v.is_null()),
        "archived board get must stamp archived_at: {archived}"
    );
}

// ============================================================================
// Completion columns on tool_update_board (the ordered list that replaces the
// legacy positional guess; first entry is the primary move target for
// status=done, an empty array disables status/column auto-sync).
// ============================================================================

async fn setup_server_with_completion_board() -> (KanbanMcpServer, TempDir, Vec<String>) {
    let (server, tmp) = setup_server().await;
    server
        .tool_create_board(Parameters(board_req("B", Some("KAN".into()))))
        .await
        .unwrap();
    let mut col_ids = Vec::new();
    for name in ["TODO", "Doing", "Done", "Decision"] {
        let result = server
            .tool_create_column(Parameters(column_req("B", name)))
            .await
            .unwrap();
        col_ids.push(text_payload(&result)["id"].as_str().unwrap().to_string());
    }
    (server, tmp, col_ids)
}

fn board_update_req(completion: Option<Vec<String>>) -> kanban_mcp::UpdateBoardRequest {
    kanban_mcp::UpdateBoardRequest {
        board: "B".to_string(),
        name: None,
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        completion_column_ids: completion,
    }
}

async fn board_completion_ids(server: &KanbanMcpServer) -> Value {
    let result = server
        .tool_get_board(Parameters(GetBoardRequest { board: "B".into() }))
        .await
        .unwrap();
    text_payload(&result)["completion_column_ids"].clone()
}

#[tokio::test]
async fn test_update_board_sets_completion_column_ids_by_name() {
    let (server, _tmp, cols) = setup_server_with_completion_board().await;

    server
        .tool_update_board(Parameters(board_update_req(Some(vec!["Done".into()]))))
        .await
        .unwrap();

    assert_eq!(
        board_completion_ids(&server).await,
        serde_json::json!([cols[2]])
    );
}

#[tokio::test]
async fn test_update_board_sets_completion_column_ids_by_uuid() {
    let (server, _tmp, cols) = setup_server_with_completion_board().await;

    server
        .tool_update_board(Parameters(board_update_req(Some(vec![cols[2].clone()]))))
        .await
        .unwrap();

    assert_eq!(
        board_completion_ids(&server).await,
        serde_json::json!([cols[2]])
    );
}

#[tokio::test]
async fn test_update_board_completion_column_ids_preserves_order() {
    let (server, _tmp, cols) = setup_server_with_completion_board().await;

    server
        .tool_update_board(Parameters(board_update_req(Some(vec![
            "Decision".into(),
            "Done".into(),
        ]))))
        .await
        .unwrap();

    assert_eq!(
        board_completion_ids(&server).await,
        serde_json::json!([cols[3], cols[2]]),
        "element 0 is the primary completion column; order must be as supplied"
    );
}

#[tokio::test]
async fn test_update_board_empty_completion_column_ids_clears_configuration() {
    let (server, _tmp, _cols) = setup_server_with_completion_board().await;

    server
        .tool_update_board(Parameters(board_update_req(Some(vec!["Done".into()]))))
        .await
        .unwrap();
    server
        .tool_update_board(Parameters(board_update_req(Some(vec![]))))
        .await
        .unwrap();

    assert_eq!(board_completion_ids(&server).await, serde_json::json!([]));
}

#[tokio::test]
async fn test_update_board_omitting_completion_column_ids_leaves_field_unchanged() {
    let (server, _tmp, cols) = setup_server_with_completion_board().await;

    server
        .tool_update_board(Parameters(board_update_req(Some(vec!["Done".into()]))))
        .await
        .unwrap();
    let mut rename_only = board_update_req(None);
    rename_only.name = Some("Renamed".into());
    server
        .tool_update_board(Parameters(rename_only))
        .await
        .unwrap();

    let result = server
        .tool_get_board(Parameters(GetBoardRequest {
            board: "Renamed".into(),
        }))
        .await
        .unwrap();
    assert_eq!(
        text_payload(&result)["completion_column_ids"],
        serde_json::json!([cols[2]]),
        "an unrelated update must not wipe the completion configuration"
    );
}

#[tokio::test]
async fn test_update_board_completion_column_of_other_board_returns_error() {
    let (server, _tmp, _cols) = setup_server_with_completion_board().await;
    server
        .tool_create_board(Parameters(board_req("Other", None)))
        .await
        .unwrap();
    let result = server
        .tool_create_column(Parameters(column_req("Other", "Elsewhere")))
        .await
        .unwrap();
    let other_col = text_payload(&result)["id"].as_str().unwrap().to_string();

    let err = server
        .tool_update_board(Parameters(board_update_req(Some(vec![other_col]))))
        .await
        .unwrap_err();

    let msg = format!("{err:?}");
    assert!(msg.contains("column"), "err: {msg}");
    assert_eq!(board_completion_ids(&server).await, serde_json::json!([]));
}

#[tokio::test]
async fn test_update_board_schema_advertises_completion_column_ids() {
    // An undiscoverable field is not shipped: the advertised JSON schema must
    // carry the field with a description covering ordering and the disable.
    let schema =
        serde_json::to_value(rmcp::schemars::schema_for!(kanban_mcp::UpdateBoardRequest)).unwrap();
    let prop = &schema["properties"]["completion_column_ids"];
    assert!(
        !prop.is_null(),
        "schema must advertise completion_column_ids: {schema}"
    );
    let description = prop["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("first") || description.contains("primary"),
        "description must explain ordering: {description}"
    );
    assert!(
        description.contains("empty"),
        "description must explain the empty-array disable: {description}"
    );
}

#[tokio::test]
async fn test_update_card_status_done_lands_in_configured_column_via_mcp() {
    // The originally reported scenario, end to end through the MCP tools: an
    // agent sets status=done and the card must land in the CONFIGURED Done
    // column (not the last column), then a move into Done keeps status=done.
    let (server, _tmp, cols) = setup_server_with_completion_board().await;
    server
        .tool_update_board(Parameters(board_update_req(Some(vec!["Done".into()]))))
        .await
        .unwrap();
    let result = server
        .tool_create_card(Parameters(card_req("B", "TODO", "Card")))
        .await
        .unwrap();
    let card_id = text_payload(&result)["id"].as_str().unwrap().to_string();

    let result = server
        .tool_update_card(Parameters(kanban_mcp::UpdateCardRequest {
            card: card_id.clone(),
            title: None,
            description: None,
            priority: None,
            status: Some("done".into()),
            due_date: None,
            clear_due_date: None,
            points: None,
        }))
        .await
        .unwrap();
    let payload = text_payload(&result);
    assert_eq!(payload["status"], "done");
    assert_eq!(
        payload["column_id"],
        serde_json::json!(cols[2]),
        "status=done must land in the configured Done column, not the last column"
    );

    let result = server
        .tool_move_card(Parameters(MoveCardRequest {
            card: card_id,
            column: "Done".into(),
            position: None,
        }))
        .await
        .unwrap();
    assert_eq!(
        text_payload(&result)["status"],
        "done",
        "moving into the configured completion column must not reset the status"
    );
}
