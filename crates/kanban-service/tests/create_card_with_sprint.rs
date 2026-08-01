use kanban_domain::{CreateCardOptions, DomainError, KanbanError, KanbanOperations, KanbanResult};
use kanban_service::{AppConfig, KanbanContext};
use tempfile::TempDir;
use uuid::Uuid;

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

#[tokio::test(flavor = "multi_thread")]
async fn create_card_with_sprint_id_in_options_assigns_card_to_sprint() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            CreateCardOptions {
                sprint_id: Some(sprint.id),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(card.sprint_id, Some(sprint.id));
    assert_eq!(card.sprint_logs.len(), 1);
    assert_eq!(card.sprint_logs[0].sprint_id, sprint.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_card_without_sprint_id_leaves_card_unassigned() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    assert_eq!(card.sprint_id, None);
    assert!(card.sprint_logs.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn create_card_with_sprint_and_undo_removes_card_and_assignment() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            CreateCardOptions {
                sprint_id: Some(sprint.id),
                ..Default::default()
            },
        )
        .unwrap();

    ctx.undo().unwrap();

    assert!(
        ctx.get_card(card.id).unwrap().is_none(),
        "Card should be gone after undo of single create-with-sprint command"
    );
}

/// Negative path: passing a sprint UUID that does not exist surfaces a
/// typed `NotFound { entity: "sprint" }` error through the full service
/// stack, not a panic or a generic validation message. Pins the
/// `get_sprint(sprint_id)?` line in `CreateCard::execute`.
#[tokio::test(flavor = "multi_thread")]
async fn create_card_with_unknown_sprint_uuid_returns_not_found() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();

    let bogus = Uuid::new_v4();
    let err = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            CreateCardOptions {
                sprint_id: Some(bogus),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert!(err.is_not_found(), "expected NotFound, got: {err:?}");
    match &err {
        KanbanError::Domain(DomainError::NotFound { entity, id }) => {
            assert_eq!(*entity, "Sprint", "wrong entity tag: {err:?}");
            assert_eq!(*id, bogus, "wrong id in NotFound: {err:?}");
        }
        other => panic!("expected NotFound, got: {other:?}"),
    }
}

/// Negative path: a sprint belonging to board B cannot be used when
/// creating a card on board A. The error is the typed
/// `SprintBoardMismatch` variant (not a stringly-typed Validation),
/// so callers and tests can match on it structurally.
#[tokio::test(flavor = "multi_thread")]
async fn create_card_with_cross_board_sprint_returns_typed_mismatch() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    let col_a = ctx.create_column(board_a.id, "Todo".into(), None).unwrap();
    let sprint_b = ctx.create_sprint(board_b.id, None, None).unwrap();

    let err = ctx
        .create_card(
            board_a.id,
            col_a.id,
            "Card".into(),
            CreateCardOptions {
                sprint_id: Some(sprint_b.id),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert!(
        err.is_sprint_board_mismatch(),
        "expected SprintBoardMismatch, got: {err:?}"
    );
    match &err {
        KanbanError::Domain(DomainError::SprintBoardMismatch {
            sprint_id,
            sprint_board,
            card_board,
        }) => {
            assert_eq!(*sprint_id, sprint_b.id);
            assert_eq!(*sprint_board, board_b.id);
            assert_eq!(*card_board, board_a.id);
        }
        other => panic!("expected SprintBoardMismatch, got: {other:?}"),
    }
}
