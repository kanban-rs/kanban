use kanban_domain::data_store::DataStore;
use kanban_domain::{
    ArchiveMetadata, ArchivedCard, Board, Card, CardPriority, CardRecord, CardStatus, Column,
    ColumnRecord,
};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

fn seed_board_and_column(store: &SqliteStore, board_name: &str) -> (Uuid, Uuid) {
    let board = Board::new(board_name, None::<String>);
    let board_id = board.id;
    store.upsert_board(board).unwrap();

    let column = Column::reconstitute(ColumnRecord {
        id: Uuid::new_v4(),
        board_id,
        name: "Todo".to_string(),
        position: 0,
        wip_limit: None,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-01-01T00:00:00Z".parse().unwrap(),
    })
    .unwrap();
    let column_id = column.id;
    store.upsert_column(column).unwrap();
    (board_id, column_id)
}

fn card_in(board_id: Uuid, column_id: Uuid, title: &str) -> Card {
    Card::reconstitute(CardRecord {
        id: Uuid::new_v4(),
        column_id,
        board_id,
        title: title.to_string(),
        description: Some("body".to_string()),
        priority: CardPriority::High,
        status: CardStatus::Done,
        position: 3,
        due_date: Some("2024-05-05T00:00:00Z".parse().unwrap()),
        points: Some(3),
        card_number: 42,
        sprint_id: None,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
        completed_at: Some("2024-03-03T00:00:00Z".parse().unwrap()),
        sprint_logs: vec![],
    })
    .unwrap()
}

#[test]
fn test_archived_card_round_trip_preserves_board_id_and_all_fields() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board_id, column_id) = seed_board_and_column(&store, "B");

        let card = card_in(board_id, column_id, "Archived");
        let card_id = card.id;
        store.upsert_card(card).unwrap();

        let ac = kanban_domain::Archived::with_context(
            card_id,
            kanban_domain::CardRestoreContext { board_id },
            ArchiveMetadata::at("2024-06-01T00:00:00Z".parse().unwrap()),
        );
        store.insert_archived_card(ac).unwrap();

        let loaded = store
            .get_archived_card(card_id)
            .unwrap()
            .expect("archived card should load");
        assert_eq!(
            loaded.context.board_id, board_id,
            "board_id must round-trip"
        );
        assert_eq!(
            loaded, ac,
            "all archived-card marker fields must round-trip"
        );

        // The live card row survives behind the marker and keeps its fields.
        let live = store
            .get_card(card_id)
            .unwrap()
            .expect("live card survives");
        assert_eq!(live.title, "Archived");
        assert_eq!(live.column_id, column_id);
    });
}

#[test]
fn test_list_archived_cards_by_board_filters_by_board_id() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board_a, col_a) = seed_board_and_column(&store, "A");
        let (board_b, col_b) = seed_board_and_column(&store, "B");

        let card_a = card_in(board_a, col_a, "A1");
        let a_id = card_a.id;
        store.upsert_card(card_a).unwrap();
        let card_b = card_in(board_b, col_b, "B1");
        let b_id = card_b.id;
        store.upsert_card(card_b).unwrap();

        let a = ArchivedCard::new(a_id, board_a);
        let b = ArchivedCard::new(b_id, board_b);
        store.insert_archived_card(a).unwrap();
        store.insert_archived_card(b).unwrap();

        let only_a = store.list_archived_cards_by_board(board_a).unwrap();
        assert_eq!(only_a.len(), 1, "only board A's archived card");
        assert_eq!(only_a[0].entity_id, a_id);
        assert!(only_a.iter().all(|ac| ac.context.board_id == board_a));
    });
}

#[test]
fn test_delete_column_does_not_cascade_delete_archived_card() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board_id, column_id) = seed_board_and_column(&store, "B");

        let card = card_in(board_id, column_id, "Survivor");
        let card_id = card.id;
        store.upsert_card(card).unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card_id, board_id))
            .unwrap();

        // Raw column delete (bypasses the domain guard) must NOT orphan the
        // archived card via the cards -> columns cascade.
        store.delete_column(column_id).unwrap();

        let survived = store
            .get_archived_card(card_id)
            .unwrap()
            .expect("archived card marker must survive its column's deletion");
        assert_eq!(survived.context.board_id, board_id);

        // The live card row survives too and retains its (now-dangling)
        // column_id, matching in-memory/JSON semantics.
        let live = store
            .get_card(card_id)
            .unwrap()
            .expect("live card row must survive its column's deletion");
        assert_eq!(
            live.column_id, column_id,
            "card.column_id is retained (dangling)"
        );
    });
}

#[test]
fn test_list_all_cards_excludes_archived_via_not_exists() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board_id, column_id) = seed_board_and_column(&store, "B");

        let live = card_in(board_id, column_id, "Live");
        let live_id = live.id;
        store.upsert_card(live).unwrap();

        let archived_card = card_in(board_id, column_id, "Archived");
        let archived_id = archived_card.id;
        store.upsert_card(archived_card).unwrap();
        store
            .insert_archived_card(ArchivedCard::new(archived_id, board_id))
            .unwrap();

        let all = store.list_all_cards().unwrap();
        let ids: Vec<Uuid> = all.iter().map(|c| c.id).collect();
        assert!(ids.contains(&live_id), "live card is listed");
        assert!(
            !ids.contains(&archived_id),
            "archived card is excluded from live listings"
        );
    });
}
