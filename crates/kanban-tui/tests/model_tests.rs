use kanban_domain::{ArchivedCard, Board, Card, Column, DependencyGraph, Snapshot, Sprint};
use kanban_tui::app::model::Model;
use uuid::Uuid;

fn make_card(board: &mut Board, column_id: Uuid, title: &str, pos: i32) -> Card {
    Card::new(board, column_id, title.to_string(), pos)
}

#[test]
fn test_empty_model_returns_empty_slices() {
    let model = Model::default();
    assert!(model.boards().is_empty());
    assert!(model.columns().is_empty());
    assert!(model.cards().is_empty());
    assert!(model.sprints().is_empty());
    assert!(model.archived_cards().is_empty());
    assert_eq!(model.graph(), &DependencyGraph::default());
}

#[test]
fn test_load_from_snapshot_populates_all_fields() {
    let mut model = Model::default();

    let mut board = Board::new("Board1", None::<String>);
    let column = Column::new(board.id, "Col1", 0);
    let card = make_card(&mut board, column.id, "Card1", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![column],
        cards: vec![card],
        archived_cards: vec![],
        sprints: vec![sprint],
        graph: DependencyGraph::default(),
    };

    model.load_from_snapshot(snapshot);

    assert_eq!(model.boards().len(), 1);
    assert_eq!(model.boards()[0].name, "Board1");
    assert_eq!(model.columns().len(), 1);
    assert_eq!(model.columns()[0].name, "Col1");
    assert_eq!(model.cards().len(), 1);
    assert_eq!(model.cards()[0].title, "Card1");
    assert_eq!(model.sprints().len(), 1);
    assert_eq!(model.sprints()[0].sprint_number, 1);
}

#[test]
fn test_card_lookup_by_id() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let column_id = Uuid::new_v4();
    let card1 = make_card(&mut board, column_id, "First", 0);
    let card2 = make_card(&mut board, column_id, "Second", 1);
    let id1 = card1.id;
    let id2 = card2.id;

    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card1, card2],
        ..Default::default()
    });

    assert_eq!(model.card_by_id(id1).unwrap().title, "First");
    assert_eq!(model.card_by_id(id2).unwrap().title, "Second");
}

#[test]
fn test_card_lookup_missing_id_returns_none() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let card = make_card(&mut board, Uuid::new_v4(), "Exists", 0);
    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card],
        ..Default::default()
    });

    assert!(model.card_by_id(Uuid::new_v4()).is_none());
}

#[test]
fn test_load_from_snapshot_rebuilds_card_index() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let column_id = Uuid::new_v4();
    let card_a = make_card(&mut board, column_id, "A", 0);
    let id_a = card_a.id;

    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card_a],
        ..Default::default()
    });
    assert!(model.card_by_id(id_a).is_some());

    let card_b = make_card(&mut board, column_id, "B", 0);
    let id_b = card_b.id;
    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card_b],
        ..Default::default()
    });

    assert!(
        model.card_by_id(id_a).is_none(),
        "old card should not be found"
    );
    assert_eq!(model.card_by_id(id_b).unwrap().title, "B");
}

// Helper mirroring the archived-cards view: the archived subset of the unified
// collection, filtered by `archived_card_ids` (the same set that backs
// `displayed_cards`).
fn archived_titles(model: &Model) -> Vec<String> {
    let ids = model.archived_card_ids();
    model
        .cards()
        .iter()
        .filter(|c| ids.contains(&c.id))
        .map(|c| c.title.clone())
        .collect()
}

#[test]
fn test_archived_cards_resolve_from_unified_collection() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let column_id = Uuid::new_v4();
    let card1 = make_card(&mut board, column_id, "Archived1", 0);
    let card2 = make_card(&mut board, column_id, "Archived2", 1);
    let ac1 = ArchivedCard::new(card1.id, uuid::Uuid::nil());
    let ac2 = ArchivedCard::new(card2.id, uuid::Uuid::nil());

    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card1, card2],
        archived_cards: vec![ac1, ac2],
        ..Default::default()
    });

    assert_eq!(archived_titles(&model), vec!["Archived1", "Archived2"]);
}

#[test]
fn test_archived_id_set_rebuilds_on_reload() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let column_id = Uuid::new_v4();

    let card1 = make_card(&mut board, column_id, "First", 0);
    let ac1 = ArchivedCard::new(card1.id, uuid::Uuid::nil());
    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card1],
        archived_cards: vec![ac1],
        ..Default::default()
    });
    assert_eq!(archived_titles(&model), vec!["First"]);

    let card2 = make_card(&mut board, column_id, "Second", 0);
    let card3 = make_card(&mut board, column_id, "Third", 1);
    let ac2 = ArchivedCard::new(card2.id, uuid::Uuid::nil());
    let ac3 = ArchivedCard::new(card3.id, uuid::Uuid::nil());
    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card2, card3],
        archived_cards: vec![ac2, ac3],
        ..Default::default()
    });

    assert_eq!(
        archived_titles(&model),
        vec!["Second", "Third"],
        "should reflect second snapshot"
    );
}

#[test]
fn test_card_by_id_resolves_archived_card() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let column_id = Uuid::new_v4();
    let card1 = make_card(&mut board, column_id, "Archived1", 0);
    let card2 = make_card(&mut board, column_id, "Archived2", 1);
    let id1 = card1.id;
    let id2 = card2.id;
    let ac1 = ArchivedCard::new(card1.id, uuid::Uuid::nil());
    let ac2 = ArchivedCard::new(card2.id, uuid::Uuid::nil());

    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card1, card2],
        archived_cards: vec![ac1, ac2],
        ..Default::default()
    });

    assert_eq!(model.card_by_id(id1).unwrap().title, "Archived1");
    assert_eq!(model.card_by_id(id2).unwrap().title, "Archived2");
    assert!(model.archived_card_ids().contains(&id1));
    assert!(model.archived_card_ids().contains(&id2));
}

#[test]
fn test_card_by_id_missing_returns_none() {
    let mut model = Model::default();

    let mut board = Board::new("B", None::<String>);
    let column_id = Uuid::new_v4();
    let card = make_card(&mut board, column_id, "Archived", 0);
    let ac = ArchivedCard::new(card.id, uuid::Uuid::nil());

    model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card],
        archived_cards: vec![ac],
        ..Default::default()
    });

    assert!(model.card_by_id(Uuid::new_v4()).is_none());
}
