use super::InMemoryStore;
use crate::{KanbanResult, Snapshot};

impl InMemoryStore {
    pub(super) fn snapshot_impl(&self) -> KanbanResult<Snapshot> {
        let state = self.read_state()?;

        let mut boards: Vec<_> = state.boards.values().cloned().collect();
        boards.sort_by_key(|b| b.position);

        let mut columns: Vec<_> = state.columns.values().cloned().collect();
        columns.sort_by_key(|c| c.position);

        let mut cards: Vec<_> = state.cards.values().cloned().collect();
        cards.sort_by_key(|c| c.position);

        let mut archived_cards: Vec<_> = state.archived_cards.values().cloned().collect();
        archived_cards.sort_by(|a, b| a.metadata.archived_at.cmp(&b.metadata.archived_at));

        let mut sprints: Vec<_> = state.sprints.values().cloned().collect();
        sprints.sort_by_key(|s| s.sprint_number);

        Ok(Snapshot::from_data(
            boards,
            columns,
            cards,
            archived_cards,
            sprints,
            state.graph.clone(),
        ))
    }

    pub(super) fn apply_snapshot_impl(&self, snapshot: Snapshot) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.boards = snapshot.boards.into_iter().map(|b| (b.id, b)).collect();
        state.columns = snapshot.columns.into_iter().map(|c| (c.id, c)).collect();
        state.cards = snapshot.cards.into_iter().map(|c| (c.id, c)).collect();
        state.rebuild_card_column_index();
        state.archived_cards = snapshot
            .archived_cards
            .into_iter()
            .map(|ac| (ac.card.id, ac))
            .collect();
        state.sprints = snapshot.sprints.into_iter().map(|s| (s.id, s)).collect();
        state.graph = snapshot.graph;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};
    use crate::{DependencyGraph, Sprint};

    #[test]
    fn test_snapshot_roundtrip() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        store.upsert_board(board).unwrap();
        store.upsert_column(col).unwrap();
        store.upsert_card(card).unwrap();
        store.upsert_sprint(sprint).unwrap();

        let snap = store.snapshot().unwrap();

        let store2 = InMemoryStore::new();
        store2.apply_snapshot(snap).unwrap();

        assert_eq!(store2.list_boards().unwrap().len(), 1);
        assert_eq!(store2.list_all_columns().unwrap().len(), 1);
        assert_eq!(store2.list_all_cards().unwrap().len(), 1);
        assert_eq!(store2.list_all_sprints().unwrap().len(), 1);
    }

    #[test]
    fn test_snapshot_sorts_entities_by_position() {
        let store = InMemoryStore::new();
        let mut board_b = make_board("B");
        board_b.position = 1;
        let mut board_a = make_board("A");
        board_a.position = 0;
        store.upsert_board(board_b.clone()).unwrap();
        store.upsert_board(board_a.clone()).unwrap();

        let col_z = make_column(board_a.id, "Z", 2);
        let col_a = make_column(board_a.id, "A", 0);
        let col_m = make_column(board_a.id, "M", 1);
        store.upsert_column(col_z).unwrap();
        store.upsert_column(col_a.clone()).unwrap();
        store.upsert_column(col_m).unwrap();

        let card3 = make_card(&mut board_a.clone(), col_a.id, "C3", 2);
        let card1 = make_card(&mut board_a.clone(), col_a.id, "C1", 0);
        store.upsert_card(card3).unwrap();
        store.upsert_card(card1).unwrap();

        let s2 = Sprint::new(board_a.id, 2, None, None::<String>);
        let s1 = Sprint::new(board_a.id, 1, None, None::<String>);
        store.upsert_sprint(s2).unwrap();
        store.upsert_sprint(s1).unwrap();

        let snap = store.snapshot().unwrap();

        assert_eq!(
            snap.boards[0].name, "A",
            "boards should be sorted by position"
        );
        assert_eq!(snap.boards[1].name, "B");
        assert_eq!(
            snap.columns[0].name, "A",
            "columns should be sorted by position"
        );
        assert_eq!(snap.columns[1].name, "M");
        assert_eq!(snap.columns[2].name, "Z");
        assert_eq!(
            snap.cards[0].title, "C1",
            "cards should be sorted by position"
        );
        assert_eq!(snap.cards[1].title, "C3");
        assert_eq!(
            snap.sprints[0].sprint_number, 1,
            "sprints should be sorted by sprint_number"
        );
        assert_eq!(snap.sprints[1].sprint_number, 2);
    }

    #[test]
    fn test_apply_snapshot_replaces_existing_data() {
        let store = InMemoryStore::new();
        let board_old = make_board("Old");
        store.upsert_board(board_old).unwrap();

        let board_new = make_board("New");
        let snap = Snapshot::from_data(
            vec![board_new],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        store.apply_snapshot(snap).unwrap();

        let boards = store.list_boards().unwrap();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "New");
    }
}
