use super::ordering::sort_by_position;
use super::InMemoryStore;
use kanban_domain::{KanbanResult, Snapshot};

impl InMemoryStore {
    pub fn snapshot_impl(&self) -> KanbanResult<Snapshot> {
        let state = self.read_state()?;

        let mut boards: Vec<_> = state.boards.values().cloned().collect();
        sort_by_position(&mut boards);

        let mut columns: Vec<_> = state.columns.values().cloned().collect();
        sort_by_position(&mut columns);

        // F3b (KAN-884): reference-marker model. Every card — live AND archived —
        // is the single source of truth in `state.cards`, so `.cards` carries them
        // all. `.archived_cards` is pure markers (`entity_id` references the card
        // in `.cards`); nothing is embedded, nothing is duplicated.
        let mut cards: Vec<_> = state.cards.values().cloned().collect();
        sort_by_position(&mut cards);

        let mut archived_cards: Vec<kanban_domain::ArchivedCard> =
            state.archived_cards.values().copied().collect();
        archived_cards.sort_by_key(|ac| ac.metadata.archived_at);

        let mut sprints: Vec<_> = state.sprints.values().cloned().collect();
        sprints.sort_by_key(|s| s.sprint_number);

        let mut archived_boards: Vec<_> = state.archived_boards.values().cloned().collect();
        archived_boards.sort_by_key(|ab| ab.metadata.archived_at);

        let mut snap = Snapshot::from_data(
            boards,
            columns,
            cards,
            archived_cards,
            sprints,
            state.graph.clone(),
        );
        snap.archived_boards = archived_boards;
        snap.prefixes = state.prefixes.clone();
        Ok(snap)
    }

    pub fn apply_snapshot_impl(&self, snapshot: Snapshot) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.boards = snapshot.boards.into_iter().map(|b| (b.id, b)).collect();
        state.columns = snapshot.columns.into_iter().map(|c| (c.id, c)).collect();
        // F3b (KAN-884): `snapshot.cards` already carries every card (live AND
        // archived — the source of truth); `archived_cards`/`archived_boards` are
        // pure markers keyed by `entity_id`. Nothing to lift.
        state.cards = snapshot.cards.into_iter().map(|c| (c.id, c)).collect();
        state.rebuild_card_column_index();
        state.archived_cards = snapshot
            .archived_cards
            .into_iter()
            .map(|ac| (ac.entity_id, ac))
            .collect();
        state.archived_boards = snapshot
            .archived_boards
            .into_iter()
            .map(|ab| (ab.entity_id, ab))
            .collect();
        state.sprints = snapshot.sprints.into_iter().map(|s| (s.id, s)).collect();
        state.graph = snapshot.graph;
        state.prefixes = snapshot.prefixes;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{DependencyGraph, Sprint};

    #[test]
    fn test_snapshot_roundtrip() {
        let store = InMemoryStore::new();
        let board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&board, col.id, "Card", 0);
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
    fn test_snapshot_round_trips_archived_boards() {
        use kanban_domain::Archived;
        let store = InMemoryStore::new();
        let live = make_board("live");
        let archived = make_board("archived");
        let archived_id = archived.id;
        store.upsert_board(live).unwrap();
        store.upsert_board(archived).unwrap();
        store
            .insert_archived_board(Archived::now(archived_id))
            .unwrap();

        let snap = store.snapshot().unwrap();
        // Reference-marker model: BOTH board heads live in `.boards` (the archived
        // one is the marker's referenced entity); `.archived_boards` holds the
        // pure marker.
        assert_eq!(snap.boards.len(), 2, "all board heads are in .boards");
        assert_eq!(snap.archived_boards.len(), 1);

        let store2 = InMemoryStore::new();
        store2.apply_snapshot(snap).unwrap();

        // list_boards is live-scoped: only the non-archived board.
        assert_eq!(store2.list_boards().unwrap().len(), 1);
        let restored = store2.list_archived_boards().unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].entity_id, archived_id);
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

        let card3 = make_card(&board_a, col_a.id, "C3", 2);
        let card1 = make_card(&board_a, col_a.id, "C1", 0);
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
    fn test_snapshot_orders_boards_with_equal_position_by_created_at() {
        use chrono::{TimeZone, Utc};
        // Many equal-position boards inserted in reverse so the snapshot's old
        // position-only sort cannot pass by luck (1/16!). See boards.rs.
        let store = InMemoryStore::new();
        let n = 16;
        for k in (0..n).rev() {
            let mut b = make_board(&format!("b{k:02}"));
            b.position = 0;
            b.created_at = Utc.timestamp_opt(1_000 + k as i64, 0).unwrap();
            store.upsert_board(b).unwrap();
        }

        let names: Vec<String> = store
            .snapshot()
            .unwrap()
            .boards
            .iter()
            .map(|b| b.name.clone())
            .collect();
        let expected: Vec<String> = (0..n).map(|k| format!("b{k:02}")).collect();

        assert_eq!(
            names, expected,
            "equal-position boards must snapshot fully ordered by created_at"
        );
    }

    #[test]
    fn test_snapshot_orders_columns_with_equal_position_by_created_at() {
        use chrono::{TimeZone, Utc};
        let store = InMemoryStore::new();
        let board = make_board("B");
        let n = 16;
        for k in (0..n).rev() {
            let mut c = make_column(board.id, &format!("c{k:02}"), 0);
            c.created_at = Utc.timestamp_opt(1_000 + k as i64, 0).unwrap();
            store.upsert_column(c).unwrap();
        }

        let names: Vec<String> = store
            .snapshot()
            .unwrap()
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect();
        let expected: Vec<String> = (0..n).map(|k| format!("c{k:02}")).collect();

        assert_eq!(
            names, expected,
            "equal-position columns must snapshot fully ordered by created_at"
        );
    }

    #[test]
    fn test_snapshot_orders_cards_with_equal_position_by_created_at() {
        use chrono::{TimeZone, Utc};
        let store = InMemoryStore::new();
        let board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let n = 16;
        for k in (0..n).rev() {
            let mut c = make_card(&board, col.id, &format!("c{k:02}"), 0);
            c.created_at = Utc.timestamp_opt(1_000 + k as i64, 0).unwrap();
            store.upsert_card(c).unwrap();
        }

        let titles: Vec<String> = store
            .snapshot()
            .unwrap()
            .cards
            .iter()
            .map(|c| c.title.clone())
            .collect();
        let expected: Vec<String> = (0..n).map(|k| format!("c{k:02}")).collect();

        assert_eq!(
            titles, expected,
            "equal-position cards must snapshot fully ordered by created_at"
        );
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
