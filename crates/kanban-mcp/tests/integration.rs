use kanban_core::AppConfig;
use kanban_domain::KanbanOperations;
use kanban_mcp::context::McpContext;
use kanban_service::StoreManager;
use tempfile::TempDir;

fn default_store_manager() -> StoreManager {
    StoreManager::new(kanban_service::default_registry())
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
    assert!(archived.iter().any(|c| c.card.id == card.id));

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

    let fresh = kanban_service::open_context(&path_str, AppConfig::default())
        .await
        .unwrap();
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

    let fresh = kanban_service::open_context(&path_str, AppConfig::default())
        .await
        .unwrap();
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

    let fresh = kanban_service::open_context(&path_str, AppConfig::default())
        .await
        .unwrap();
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
    AssignCardToSprintRequest, CarryOverSprintCardsRequest, CreateBoardRequest, CreateCardRequest,
    CreateColumnRequest, CreateSprintRequest, KanbanMcpServer, MoveCardRequest, MoveCardsRequest,
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
        completion_column_id: None,
    }
}

/// Minimal-path column-create request (KAN-794): the `board` name plus the
/// shared `kanban_service::api::CreateColumnRequest` content with no client id
/// and no wip_limit. Position is server-assigned on append (not a create field).
fn column_req(board: &str, name: &str) -> CreateColumnRequest {
    CreateColumnRequest {
        board: board.to_string(),
        content: kanban_service::api::CreateColumnRequest {
            id: None,
            name: name.to_string(),
            wip_limit: None,
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
        .tool_create_card(Parameters(CreateCardRequest {
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
            .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "Alpha".into(),
            prefix: None,
            name: Some("completed".into()),
        }))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "Alpha".into(),
            prefix: None,
            name: Some("next".into()),
        }))
        .await
        .unwrap();
    server
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "Beta".into(),
            prefix: None,
            name: Some("next".into()),
        }))
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
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "B".into(),
            prefix: None,
            name: Some("alpha".into()),
        }))
        .await
        .unwrap();
    server
        .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_card(Parameters(CreateCardRequest {
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
        }))
        .await
        .unwrap();
    let listed_body = text_payload(&listed);
    let parents = listed_body.as_array().expect("array");
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
        }))
        .await
        .unwrap();
    let arr = text_payload(&listed);
    let parents = arr.as_array().expect("array");
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0]["title"], "Parent");
    assert!(parents[0]["id"].is_string());

    let children = server
        .tool_list_card_children(Parameters(ListCardChildrenRequest {
            card: parent.clone(),
        }))
        .await
        .unwrap();
    let arr = text_payload(&children);
    let cs = arr.as_array().expect("array");
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0]["title"], "Child");
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
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "B".into(),
            prefix: None,
            name: Some("alpha".into()),
        }))
        .await
        .unwrap();
    let sprint_body = text_payload(&sprint_result);
    let sprint_id = sprint_body["id"].as_str().unwrap().to_string();

    let result = server
        .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "B".into(),
            prefix: None,
            name: Some("alpha".into()),
        }))
        .await
        .unwrap();
    let result = server
        .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_card(Parameters(CreateCardRequest {
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
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "B".into(),
            prefix: None,
            name: Some("beta".into()),
        }))
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
        .tool_create_card(Parameters(CreateCardRequest {
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
        completion_column_id: None,
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
    let req = CreateColumnRequest {
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
    let req = CreateColumnRequest {
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
fn card_req(board: &str, column: &str, title: &str) -> CreateCardRequest {
    CreateCardRequest {
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
    let req = CreateCardRequest {
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
        .tool_create_sprint(Parameters(CreateSprintRequest {
            board: "B".into(),
            prefix: None,
            name: Some("alpha".into()),
        }))
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
