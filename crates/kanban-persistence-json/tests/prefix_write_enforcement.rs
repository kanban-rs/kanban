use kanban_backend::KanbanBackend;
use kanban_domain::{Board, Card, Column, DataStore, DomainError, KanbanError, Prefix};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use std::sync::Arc;
use tempfile::tempdir;

fn make_store(path: &std::path::Path) -> JsonDataStore {
    JsonDataStore::new(Arc::new(JsonFileStore::new(path)))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_writing_a_card_with_an_unbacked_namespace_is_rejected() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();

    let result = jds.with_transaction(Box::new(|| {
        let mut card = Card::new(board.id, col.id, "C", 0);
        card.prefix = "KAN".into();
        card.card_number = 1;
        jds.upsert_card(card)
    }));

    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_rejected_batch_leaves_the_store_unchanged() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();
    jds.upsert_prefix(Prefix::new("kan")).unwrap();
    let mut existing = Card::new(board.id, col.id, "Existing", 0);
    existing.prefix = "KAN".into();
    existing.card_number = 1;
    jds.upsert_card(existing.clone()).unwrap();
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, None::<String>);
    jds.upsert_sprint(sprint).unwrap();
    let mut archived_source = Card::new(board.id, col.id, "Archived", 1);
    archived_source.prefix = "KAN".into();
    archived_source.card_number = 2;
    jds.upsert_card(archived_source.clone()).unwrap();
    jds.insert_archived_card(kanban_domain::ArchivedCard::new(
        archived_source.id,
        board.id,
    ))
    .unwrap();
    let mut other_source = Card::new(board.id, col.id, "Other", 2);
    other_source.prefix = "KAN".into();
    other_source.card_number = 3;
    jds.upsert_card(other_source.clone()).unwrap();
    jds.modify_graph(Box::new(move |graph| {
        graph.set_block(existing.id, other_source.id)
    }))
    .unwrap();

    let before = jds.snapshot().unwrap();

    let injected = Board::new("Injected", None::<String>);
    let injected_id = injected.id;
    let offender_id = uuid::Uuid::new_v4();
    let result = jds.with_transaction(Box::new(|| {
        jds.upsert_board(injected.clone())?;
        let mut offender = Card::new(board.id, col.id, "Offender", 3);
        offender.id = offender_id;
        offender.prefix = "OPS".into();
        offender.card_number = 99;
        jds.upsert_card(offender)
    }));

    assert!(result.is_err());
    assert_eq!(jds.snapshot().unwrap(), before);
    assert!(jds.get_card(offender_id).unwrap().is_none());
    assert!(jds.get_board(injected_id).unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_the_rejection_is_the_prefix_not_backed_error() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();

    let result = jds.with_transaction(Box::new(|| {
        let mut card_a = Card::new(board.id, col.id, "A", 0);
        card_a.prefix = "aaa".into();
        card_a.card_number = 7;
        jds.upsert_card(card_a)?;
        let mut card_b = Card::new(board.id, col.id, "B", 1);
        card_b.prefix = "ZZZ".into();
        card_b.card_number = 3;
        jds.upsert_card(card_b)
    }));

    let err = result.unwrap_err();
    assert!(matches!(
        &err,
        KanbanError::Domain(DomainError::PrefixNotBacked { card_number: 3, prefix })
            if prefix == "ZZZ"
    ));
    assert_eq!(
        err.to_string(),
        "card 3 names prefix 'ZZZ', which has no row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_partly_backed_batch_is_rejected_whole() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();

    let kan_card_id = uuid::Uuid::new_v4();
    let ops_card_id = uuid::Uuid::new_v4();
    let result = jds.with_transaction(Box::new(|| {
        jds.upsert_prefix(Prefix::new("kan"))?;
        let mut kan_card = Card::new(board.id, col.id, "K", 0);
        kan_card.id = kan_card_id;
        kan_card.prefix = "KAN".into();
        kan_card.card_number = 1;
        jds.upsert_card(kan_card)?;
        let mut ops_card = Card::new(board.id, col.id, "O", 1);
        ops_card.id = ops_card_id;
        ops_card.prefix = "OPS".into();
        ops_card.card_number = 2;
        jds.upsert_card(ops_card)
    }));

    let err = result.unwrap_err();
    assert!(matches!(
        &err,
        KanbanError::Domain(DomainError::PrefixNotBacked { prefix, .. }) if prefix == "OPS"
    ));
    assert!(jds.get_card(kan_card_id).unwrap().is_none());
    assert!(jds.get_card(ops_card_id).unwrap().is_none());
    assert!(jds.get_prefix("kan").unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_batch_whose_namespace_is_backed_commits() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();
    jds.upsert_prefix(Prefix::new("kan")).unwrap();

    let card_id = uuid::Uuid::new_v4();
    let result = jds.with_transaction(Box::new(|| {
        let mut card = Card::new(board.id, col.id, "C", 0);
        card.id = card_id;
        card.prefix = "KAN".into();
        card.card_number = 1;
        jds.upsert_card(card)
    }));

    assert!(result.is_ok());
    assert!(jds.get_card(card_id).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_row_created_earlier_in_the_same_batch_backs_the_card() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();

    let card_id = uuid::Uuid::new_v4();
    let result = jds.with_transaction(Box::new(|| {
        jds.upsert_prefix(Prefix::new("kan"))?;
        let mut card = Card::new(board.id, col.id, "C", 0);
        card.id = card_id;
        card.prefix = "KAN".into();
        card.card_number = 1;
        jds.upsert_card(card)
    }));

    assert!(result.is_ok());
    assert!(jds.get_card(card_id).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_card_with_an_empty_prefix_is_accepted() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();

    let card_id = uuid::Uuid::new_v4();
    let result = jds.with_transaction(Box::new(|| {
        let mut card = Card::new(board.id, col.id, "C", 0);
        card.id = card_id;
        jds.upsert_card(card)
    }));

    assert!(result.is_ok());
    assert!(jds.get_card(card_id).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_direct_upsert_outside_a_batch_is_not_rejected() {
    let dir = tempdir().unwrap();
    let jds = make_store(&dir.path().join("t.json"));
    let board = Board::new("Board", None::<String>);
    jds.upsert_board(board.clone()).unwrap();
    let col = Column::new(board.id, "Col", 0);
    jds.upsert_column(col.clone()).unwrap();

    let mut card = Card::new(board.id, col.id, "C", 0);
    card.prefix = "KAN".into();
    card.card_number = 1;
    let card_id = card.id;
    let result = jds.upsert_card(card);

    assert!(result.is_ok());
    assert!(jds.get_card(card_id).unwrap().is_some());
}
