use kanban_domain::{CardStatus, CardUpdate, CreateCardOptions, KanbanOperations, KanbanResult};
use kanban_service::{AppConfig, KanbanContext};
use tempfile::TempDir;

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
async fn carry_over_skips_done_cards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Test Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Backlog".into(), None).unwrap();

    let from_sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let to_sprint = ctx.create_sprint(board.id, None, None).unwrap();

    // Activate then complete the from_sprint so it's eligible
    ctx.activate_sprint(from_sprint.id, Some(14)).unwrap();
    ctx.complete_sprint(from_sprint.id).unwrap();

    // Create three cards and assign them to from_sprint
    let card_todo = ctx
        .create_card(
            board.id,
            col.id,
            "Todo card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card_todo.id, from_sprint.id)
        .unwrap();

    let card_in_progress = ctx
        .create_card(
            board.id,
            col.id,
            "In Progress card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card_in_progress.id, from_sprint.id)
        .unwrap();

    let card_done = ctx
        .create_card(
            board.id,
            col.id,
            "Done card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card_done.id, from_sprint.id)
        .unwrap();

    // Mark the done card as Done
    ctx.update_card(
        card_done.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    let count = ctx
        .carry_over_sprint_cards(from_sprint.id, to_sprint.id)
        .unwrap();

    // Only the two non-Done cards should be moved
    assert_eq!(count, 2);

    let moved_todo = ctx.get_card(card_todo.id).unwrap().unwrap();
    assert_eq!(moved_todo.sprint_id, Some(to_sprint.id));

    let moved_in_progress = ctx.get_card(card_in_progress.id).unwrap().unwrap();
    assert_eq!(moved_in_progress.sprint_id, Some(to_sprint.id));

    let done_card = ctx.get_card(card_done.id).unwrap().unwrap();
    assert_eq!(done_card.sprint_id, Some(from_sprint.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn carry_over_returns_zero_when_sprint_has_no_cards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Test Board".into(), None).unwrap();
    let from_sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let to_sprint = ctx.create_sprint(board.id, None, None).unwrap();

    ctx.activate_sprint(from_sprint.id, Some(14)).unwrap();
    ctx.complete_sprint(from_sprint.id).unwrap();

    let count = ctx
        .carry_over_sprint_cards(from_sprint.id, to_sprint.id)
        .unwrap();

    assert_eq!(count, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn carry_over_returns_zero_when_all_cards_are_done() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Test Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Done".into(), None).unwrap();

    let from_sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let to_sprint = ctx.create_sprint(board.id, None, None).unwrap();

    ctx.activate_sprint(from_sprint.id, Some(14)).unwrap();
    ctx.complete_sprint(from_sprint.id).unwrap();

    let card1 = ctx
        .create_card(
            board.id,
            col.id,
            "Done card 1".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card1.id, from_sprint.id).unwrap();
    ctx.update_card(
        card1.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    let card2 = ctx
        .create_card(
            board.id,
            col.id,
            "Done card 2".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card2.id, from_sprint.id).unwrap();
    ctx.update_card(
        card2.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    let count = ctx
        .carry_over_sprint_cards(from_sprint.id, to_sprint.id)
        .unwrap();

    assert_eq!(count, 0);

    let c1 = ctx.get_card(card1.id).unwrap().unwrap();
    assert_eq!(c1.sprint_id, Some(from_sprint.id));
    let c2 = ctx.get_card(card2.id).unwrap().unwrap();
    assert_eq!(c2.sprint_id, Some(from_sprint.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn carry_over_includes_blocked_cards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.kanban").to_string_lossy().to_string();

    let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();

    let board = ctx.create_board("Test Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Backlog".into(), None).unwrap();

    let from_sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let to_sprint = ctx.create_sprint(board.id, None, None).unwrap();

    ctx.activate_sprint(from_sprint.id, Some(14)).unwrap();
    ctx.complete_sprint(from_sprint.id).unwrap();

    let card_blocked = ctx
        .create_card(
            board.id,
            col.id,
            "Blocked card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card_blocked.id, from_sprint.id)
        .unwrap();
    ctx.update_card(
        card_blocked.id,
        CardUpdate {
            status: Some(CardStatus::Blocked),
            ..Default::default()
        },
    )
    .unwrap();

    let card_done = ctx
        .create_card(
            board.id,
            col.id,
            "Done card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card_done.id, from_sprint.id)
        .unwrap();
    ctx.update_card(
        card_done.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    let count = ctx
        .carry_over_sprint_cards(from_sprint.id, to_sprint.id)
        .unwrap();

    assert_eq!(count, 1);

    let moved = ctx.get_card(card_blocked.id).unwrap().unwrap();
    assert_eq!(moved.sprint_id, Some(to_sprint.id));

    let stayed = ctx.get_card(card_done.id).unwrap().unwrap();
    assert_eq!(stayed.sprint_id, Some(from_sprint.id));
}
