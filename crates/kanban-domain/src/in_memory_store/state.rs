use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, Sprint};

#[derive(Debug, Clone)]
pub(super) struct StoreState {
    pub(super) boards: HashMap<Uuid, Board>,
    pub(super) columns: HashMap<Uuid, Column>,
    pub(super) cards: HashMap<Uuid, Card>,
    /// Secondary index: column_id -> set of card ids currently in that column.
    /// Maintained transactionally by every method that mutates `cards`'s
    /// column membership (upsert_card, delete_card, delete_cards_by_columns,
    /// apply_snapshot). Lets `count_cards_in_column_excluding` and friends
    /// run in O(column_size) instead of O(total_cards).
    pub(super) cards_by_column: HashMap<Uuid, HashSet<Uuid>>,
    pub(super) sprints: HashMap<Uuid, Sprint>,
    pub(super) archived_cards: HashMap<Uuid, ArchivedCard>,
    /// Discrete archived-board collection (C2). Archiving a board moves its
    /// head out of `boards` into here as `Archived<Board>`; its subtree
    /// (columns/cards/sprints) stays in place in the flat collections.
    pub(super) archived_boards: HashMap<Uuid, ArchivedBoard>,
    pub(super) graph: DependencyGraph,
}

impl StoreState {
    pub(super) fn new() -> Self {
        Self {
            boards: HashMap::new(),
            columns: HashMap::new(),
            cards: HashMap::new(),
            cards_by_column: HashMap::new(),
            sprints: HashMap::new(),
            archived_cards: HashMap::new(),
            archived_boards: HashMap::new(),
            graph: DependencyGraph::new(),
        }
    }

    /// F1 (KAN-870): a card is archived iff it has a marker in `archived_cards`.
    /// The card itself stays in `cards` (reference model), so live reads must
    /// consult this to hide archived cards, and `delete_card` no-ops on them.
    pub(super) fn is_card_archived(&self, card_id: &Uuid) -> bool {
        self.archived_cards.contains_key(card_id)
    }

    pub(super) fn add_card_to_column_index(&mut self, card_id: Uuid, column_id: Uuid) {
        self.cards_by_column
            .entry(column_id)
            .or_default()
            .insert(card_id);
    }

    pub(super) fn remove_card_from_column_index(&mut self, card_id: Uuid, column_id: Uuid) {
        if let Some(set) = self.cards_by_column.get_mut(&column_id) {
            set.remove(&card_id);
            if set.is_empty() {
                self.cards_by_column.remove(&column_id);
            }
        }
    }

    pub(super) fn rebuild_card_column_index(&mut self) {
        self.cards_by_column.clear();
        for card in self.cards.values() {
            self.cards_by_column
                .entry(card.column_id)
                .or_default()
                .insert(card.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};
    use crate::{ArchivedCard, DependencyGraph, InMemoryStore, Snapshot, Sprint};
    use uuid::Uuid;

    #[test]
    fn test_all_data_store_methods_return_ok_not_panic() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let ac = ArchivedCard::new(card.clone(), uuid::Uuid::nil(), col.id, 0);

        assert!(store.upsert_board(board.clone()).is_ok());
        assert!(store.get_board(board.id).is_ok());
        assert!(store.list_boards().is_ok());
        assert!(store.upsert_column(col.clone()).is_ok());
        assert!(store.get_column(col.id).is_ok());
        assert!(store.list_columns_by_board(board.id).is_ok());
        assert!(store.list_all_columns().is_ok());
        assert!(store.upsert_card(card.clone()).is_ok());
        assert!(store.get_card(card.id).is_ok());
        assert!(store.list_all_cards().is_ok());
        assert!(store.list_cards_by_column(col.id).is_ok());
        assert!(store.list_cards_by_sprint(Uuid::new_v4()).is_ok());
        assert!(store.count_cards_in_column(col.id).is_ok());
        assert!(store.count_cards_in_column_excluding(col.id, &[]).is_ok());
        assert!(store
            .clear_sprint_from_cards(Uuid::new_v4(), chrono::Utc::now())
            .is_ok());
        assert!(store.insert_archived_card(ac).is_ok());
        assert!(store.get_archived_card(card.id).is_ok());
        assert!(store.list_archived_cards().is_ok());
        assert!(store.delete_archived_card(card.id).is_ok());
        assert!(store.upsert_sprint(sprint.clone()).is_ok());
        assert!(store.get_sprint(sprint.id).is_ok());
        assert!(store.list_sprints_by_board(board.id).is_ok());
        assert!(store.list_all_sprints().is_ok());
        assert!(store.get_graph().is_ok());
        assert!(store.set_graph(DependencyGraph::new()).is_ok());
        assert!(store.snapshot().is_ok());
        assert!(store.apply_snapshot(Snapshot::new()).is_ok());
        assert!(store.delete_card(card.id).is_ok());
        assert!(store.delete_cards_by_columns(&[col.id]).is_ok());
        assert!(store.delete_column(col.id).is_ok());
        assert!(store.delete_columns_by_board(board.id).is_ok());
        assert!(store.delete_sprint(sprint.id).is_ok());
        assert!(store.delete_sprints_by_board(board.id).is_ok());
        assert!(store.delete_board(board.id).is_ok());
    }

    #[test]
    fn test_concurrent_reads_and_writes_no_panic() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(InMemoryStore::new());
        let mut handles = vec![];

        for i in 0..10 {
            let s = Arc::clone(&store);
            handles.push(thread::spawn(move || {
                let board = make_board(&format!("Board-{i}"));
                s.upsert_board(board.clone()).unwrap();
                let col = make_column(board.id, &format!("Col-{i}"), i);
                s.upsert_column(col).unwrap();
            }));
        }

        for _ in 0..10 {
            let s = Arc::clone(&store);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _ = s.list_boards();
                    let _ = s.list_all_columns();
                    let _ = s.list_all_cards();
                    let _ = s.snapshot();
                }
            }));
        }

        for h in handles {
            h.join().expect("thread should not panic");
        }

        let boards = store.list_boards().unwrap();
        assert_eq!(boards.len(), 10);
    }
}
