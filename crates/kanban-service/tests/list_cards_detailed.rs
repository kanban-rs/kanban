use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::{ArchivedFilter, CardListFilter, CreateCardOptions, KanbanOperations};
use kanban_service::KanbanContext;
use std::sync::Arc;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn test_list_cards_detailed_returns_full_cards_with_description_and_prefix() {
    let mut ctx = make_ctx().await;
    let board = ctx
        .create_board("Test Board".to_string(), Some("TB".to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Column".to_string(), None)
        .unwrap();
    ctx.create_card(
        board.id,
        column.id,
        "Card 1".to_string(),
        CreateCardOptions {
            description: Some("A description".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let pairs = ctx
        .list_cards_detailed(CardListFilter {
            board_id: Some(board.id),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(pairs.len(), 1);
    let (card, archived_at) = &pairs[0];
    assert_eq!(card.description.as_deref(), Some("A description"));
    assert_eq!(card.board_id, board.id);
    assert!(!card.prefix.is_empty());
    assert_eq!(*archived_at, None);
}

#[tokio::test]
async fn test_list_cards_detailed_pairs_archived_card_with_its_archived_at() {
    let mut ctx = make_ctx().await;
    let board = ctx
        .create_board("Test Board".to_string(), Some("TB".to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Column".to_string(), None)
        .unwrap();
    let live = ctx
        .create_card(board.id, column.id, "Live".to_string(), Default::default())
        .unwrap();
    let archived = ctx
        .create_card(
            board.id,
            column.id,
            "Archived".to_string(),
            Default::default(),
        )
        .unwrap();
    ctx.archive_card(archived.id).unwrap();

    let pairs = ctx
        .list_cards_detailed(CardListFilter {
            board_id: Some(board.id),
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();

    let archived_pair = pairs.iter().find(|(c, _)| c.id == archived.id).unwrap();
    let live_pair = pairs.iter().find(|(c, _)| c.id == live.id).unwrap();

    assert_eq!(archived_pair.1, ctx.card_archived_at(archived.id).unwrap());
    assert!(archived_pair.1.is_some());
    assert_eq!(live_pair.1, None);
}

#[tokio::test]
async fn test_list_cards_detailed_and_list_cards_agree_on_ids_and_order() {
    let mut ctx = make_ctx().await;
    let board = ctx
        .create_board("Test Board".to_string(), Some("TB".to_string()))
        .unwrap();
    let col1 = ctx
        .create_column(board.id, "Column 1".to_string(), None)
        .unwrap();
    let col2 = ctx
        .create_column(board.id, "Column 2".to_string(), None)
        .unwrap();
    ctx.create_card(board.id, col1.id, "Card A".to_string(), Default::default())
        .unwrap();
    ctx.create_card(board.id, col1.id, "Card B".to_string(), Default::default())
        .unwrap();
    ctx.create_card(board.id, col2.id, "Card C".to_string(), Default::default())
        .unwrap();

    let filter = CardListFilter {
        board_id: Some(board.id),
        ..Default::default()
    };

    let detailed = ctx.list_cards_detailed(filter.clone()).unwrap();
    let summaries = ctx.list_cards(filter).unwrap();

    let detailed_ids: Vec<_> = detailed.iter().map(|(c, _)| c.id).collect();
    let summary_ids: Vec<_> = summaries.iter().map(|s| s.id).collect();
    assert_eq!(detailed_ids, summary_ids);

    let detailed_at: Vec<_> = detailed.iter().map(|(_, at)| *at).collect();
    let summary_at: Vec<_> = summaries.iter().map(|s| s.archived_at).collect();
    assert_eq!(detailed_at, summary_at);
}
