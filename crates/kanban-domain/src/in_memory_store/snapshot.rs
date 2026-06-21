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
        archived_cards.sort_by(|a, b| a.archived_at.cmp(&b.archived_at));

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
