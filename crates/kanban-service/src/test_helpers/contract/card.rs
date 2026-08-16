use super::super::BackendFactory;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::card::{CardPriority, CardStatus};
use kanban_domain::{BoardUpdate, CardUpdate, CreateCardOptions, FieldUpdate, KanbanOperations};
use tempfile::TempDir;

pub async fn test_card_all_fields_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("FB".into())).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Full Card".into(),
            CreateCardOptions {
                description: Some("A description".into()),
                priority: Some(CardPriority::Critical),
                points: Some(8),
                due_date: Some(chrono::Utc::now()),
                ..Default::default()
            },
        )
        .unwrap();

    ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    ctx.update_card(
        card.id,
        CardUpdate {
            status: Some(CardStatus::InProgress),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(c.title, "Full Card");
    assert_eq!(c.description.as_deref(), Some("A description"));
    assert_eq!(c.priority, CardPriority::Critical);
    assert_eq!(c.status, CardStatus::InProgress);
    assert_eq!(c.column_id, col.id);
    assert_eq!(c.sprint_id, Some(sprint.id));
    assert_eq!(c.points, Some(8));
    assert!(c.due_date.is_some());
    assert!(c.card_number > 0);
    assert!(c.completed_at.is_none());
}

pub async fn test_card_minimal_fields_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Minimal".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(c.title, "Minimal");
    assert!(c.description.is_none());
    assert_eq!(c.priority, CardPriority::Medium);
    assert_eq!(c.status, CardStatus::Todo);
    assert!(c.sprint_id.is_none());
    assert!(c.points.is_none());
    assert!(c.due_date.is_none());
    assert!(c.completed_at.is_none());
    assert!(c.sprint_logs.is_empty());
}

pub async fn test_card_all_priority_variants_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let priorities = [
        CardPriority::Low,
        CardPriority::Medium,
        CardPriority::High,
        CardPriority::Critical,
    ];

    let mut card_ids = Vec::new();
    for p in &priorities {
        let card = ctx
            .create_card(
                board.id,
                col.id,
                format!("{:?} card", p),
                CreateCardOptions {
                    priority: Some(*p),
                    ..Default::default()
                },
            )
            .unwrap();
        card_ids.push(card.id);
    }

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    for (id, expected) in card_ids.iter().zip(priorities.iter()) {
        let c = ctx.get_card(*id).unwrap().unwrap();
        assert_eq!(c.priority, *expected);
    }
}

pub async fn test_card_all_status_variants_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let statuses = [
        CardStatus::Todo,
        CardStatus::InProgress,
        CardStatus::Blocked,
        CardStatus::Done,
    ];

    let mut card_ids = Vec::new();
    for s in &statuses {
        let card = ctx
            .create_card(
                board.id,
                col.id,
                format!("{:?} card", s),
                CreateCardOptions::default(),
            )
            .unwrap();
        ctx.update_card(
            card.id,
            CardUpdate {
                status: Some(*s),
                ..Default::default()
            },
        )
        .unwrap();
        card_ids.push(card.id);
    }

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    for (id, expected) in card_ids.iter().zip(statuses.iter()) {
        let c = ctx.get_card(*id).unwrap().unwrap();
        assert_eq!(c.status, *expected);
    }
}

/// F1c (KAN-926): the 3-state archived-aware primitives
/// `list_cards_by_column_filtered` / `count_cards_in_column_filtered` must agree
/// on ONE spec across every backend (in-memory, JSON, SQLite) — reached through
/// `ctx.data_store()`. Seeds 2 live + 2 archived cards in one column and pins
/// LiveOnly=2, ArchivedOnly=2, Include=4 for both list (by id) and count.
pub async fn test_column_filtered_reads_three_state(factory: &BackendFactory) {
    use kanban_domain::ArchivedFilter;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let mut live = Vec::new();
    for i in 0..2 {
        let c = ctx
            .create_card(
                board.id,
                col.id,
                format!("live{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        live.push(c.id);
    }
    let mut archived = Vec::new();
    for i in 0..2 {
        let c = ctx
            .create_card(
                board.id,
                col.id,
                format!("arch{i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        ctx.archive_card(c.id).unwrap();
        archived.push(c.id);
    }

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let ds = ctx.data_store();

    // Counts.
    assert_eq!(
        ds.count_cards_in_column_filtered(col.id, ArchivedFilter::LiveOnly)
            .unwrap(),
        2,
        "LiveOnly count == 2"
    );
    assert_eq!(
        ds.count_cards_in_column_filtered(col.id, ArchivedFilter::ArchivedOnly)
            .unwrap(),
        2,
        "ArchivedOnly count == 2"
    );
    assert_eq!(
        ds.count_cards_in_column_filtered(col.id, ArchivedFilter::Include)
            .unwrap(),
        4,
        "Include count == 4"
    );

    // Lists (compare id sets).
    let ids = |archived_filter| -> Vec<uuid::Uuid> {
        let mut v: Vec<uuid::Uuid> = ds
            .list_cards_by_column_filtered(col.id, archived_filter)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        v.sort();
        v
    };

    let mut want_live = live.clone();
    want_live.sort();
    assert_eq!(ids(ArchivedFilter::LiveOnly), want_live, "LiveOnly list");

    let mut want_archived = archived.clone();
    want_archived.sort();
    assert_eq!(
        ids(ArchivedFilter::ArchivedOnly),
        want_archived,
        "ArchivedOnly list"
    );

    let mut want_all: Vec<uuid::Uuid> = live.into_iter().chain(archived).collect();
    want_all.sort();
    assert_eq!(ids(ArchivedFilter::Include), want_all, "Include list");
}

pub async fn test_card_completed_at_set_on_done_status(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.update_card(
        card.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(c.status, CardStatus::Done);
    assert!(c.completed_at.is_some());
}

pub async fn test_get_card_by_board_and_number_returns_matching_card(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board_a = ctx.create_board("Board A".into(), None).unwrap();
    let col_a = ctx.create_column(board_a.id, "Col".into(), None).unwrap();
    let board_b = ctx.create_board("Board B".into(), None).unwrap();
    let col_b = ctx.create_column(board_b.id, "Col".into(), None).unwrap();

    let card_a1 = ctx
        .create_card(
            board_a.id,
            col_a.id,
            "A1".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_a2 = ctx
        .create_card(
            board_a.id,
            col_a.id,
            "A2".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card_b1 = ctx
        .create_card(
            board_b.id,
            col_b.id,
            "B1".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let found = ctx
        .data_store()
        .get_card_by_board_and_number(board_a.id, card_a1.card_number)
        .unwrap();
    assert_eq!(found.map(|c| c.id), Some(card_a1.id));

    let found = ctx
        .data_store()
        .get_card_by_board_and_number(board_a.id, card_a2.card_number)
        .unwrap();
    assert_eq!(found.map(|c| c.id), Some(card_a2.id));

    let found = ctx
        .data_store()
        .get_card_by_board_and_number(board_b.id, card_b1.card_number)
        .unwrap();
    assert_eq!(found.map(|c| c.id), Some(card_b1.id));
}

pub async fn test_get_card_by_board_and_number_returns_none_for_missing_number(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    ctx.create_card(
        board.id,
        col.id,
        "Card".into(),
        CreateCardOptions::default(),
    )
    .unwrap();

    let found = ctx
        .data_store()
        .get_card_by_board_and_number(board.id, 9999)
        .unwrap();
    assert!(found.is_none());
}

pub async fn test_get_card_by_sprint_and_number_returns_matching_card(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let card1 = ctx
        .create_card(
            board.id,
            col.id,
            "Card 1".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card2 = ctx
        .create_card(
            board.id,
            col.id,
            "Card 2".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.assign_card_to_sprint(card1.id, sprint.id).unwrap();
    ctx.assign_card_to_sprint(card2.id, sprint.id).unwrap();

    let card1 = ctx.get_card(card1.id).unwrap().unwrap();
    let card2 = ctx.get_card(card2.id).unwrap().unwrap();

    let found = ctx
        .data_store()
        .get_card_by_sprint_and_number(sprint.id, card1.card_number)
        .unwrap();
    assert_eq!(found.map(|c| c.id), Some(card1.id));

    let found = ctx
        .data_store()
        .get_card_by_sprint_and_number(sprint.id, card2.card_number)
        .unwrap();
    assert_eq!(found.map(|c| c.id), Some(card2.id));

    // A second sprint on a second board, holding a card whose number COLLIDES
    // with card1's. Without this foil an implementation that ignored sprint_id
    // entirely and matched on card_number alone would still pass every
    // assertion above.
    //
    // The second board needs its OWN prefix. Boards sharing a namespace share
    // one counter and so cannot produce a duplicate number at all -- which is
    // the point of the prefix row, and which would silently disarm this foil.
    let board_b = ctx
        .create_board("Board B".into(), Some("FOIL".into()))
        .unwrap();
    let col_b = ctx.create_column(board_b.id, "Col".into(), None).unwrap();
    let sprint_b = ctx.create_sprint(board_b.id, None, None).unwrap();
    let card_b = ctx
        .create_card(
            board_b.id,
            col_b.id,
            "Colliding number".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card_b.id, sprint_b.id).unwrap();
    let card_b = ctx.get_card(card_b.id).unwrap().unwrap();
    assert_eq!(
        card_b.card_number, card1.card_number,
        "the foil only works if the numbers actually collide"
    );

    let found = ctx
        .data_store()
        .get_card_by_sprint_and_number(sprint.id, card1.card_number)
        .unwrap();
    assert_eq!(
        found.map(|c| c.id),
        Some(card1.id),
        "must return the card in the requested sprint, not the colliding number in another"
    );

    let found = ctx
        .data_store()
        .get_card_by_sprint_and_number(sprint_b.id, card_b.card_number)
        .unwrap();
    assert_eq!(found.map(|c| c.id), Some(card_b.id));
}

pub async fn test_get_card_by_sprint_and_number_returns_none_for_missing_number(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();

    let found = ctx
        .data_store()
        .get_card_by_sprint_and_number(sprint.id, 9999)
        .unwrap();
    assert!(found.is_none());
}

/// A card's prefix is part of its identity and must survive storage on every
/// backend. A backend that dropped it would leave the card addressable only by
/// bare number, and KAN-1215 resolves by `(prefix, card_number)`.
pub async fn test_card_prefix_roundtrips(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("KAN".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "A card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(
        card.prefix, "kan",
        "a new card stores its board's prefix, normalised"
    );

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let reloaded = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(
        reloaded.prefix, "kan",
        "the stored prefix must survive a save and reopen"
    );
}

/// Renaming a board must not rename cards it already minted. This is the whole
/// point of storing the prefix rather than resolving it.
pub async fn test_existing_card_prefix_is_unchanged_by_a_board_rename(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("KAN".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "A card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.update_board(
        board.id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set("DEV".into()),
            ..Default::default()
        },
    )
    .unwrap();
    ctx.save().await.unwrap();

    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let reloaded = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(
        reloaded.prefix, "kan",
        "the card keeps the prefix it was minted under; the rename affects only \
         cards created afterwards"
    );
}
