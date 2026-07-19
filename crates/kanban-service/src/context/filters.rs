use super::KanbanContext;
use kanban_domain::{Card, CardListFilter, KanbanResult};
use uuid::Uuid;

impl KanbanContext {
    pub(super) fn filter_cards(&self, filter: &CardListFilter) -> KanbanResult<Vec<Card>> {
        // C10a: an explicit `board_id` is a deliberate scoped request, so base the
        // card set on THAT board's own cards (raw) — honoring the board whether it
        // is live or archived. Only the UNSCOPED read stays live-scoped (C3b).
        let (cards, columns, board) = match filter.board_id {
            Some(bid) => {
                let columns = self.backend.list_columns_by_board(bid)?;
                let col_ids: Vec<Uuid> = columns.iter().map(|c| c.id).collect();
                let cards = self.backend.list_cards_by_columns(&col_ids)?;
                // `get_board` is unfiltered (reference-marker model): it resolves
                // the head whether the board is live or archived.
                let board = self.backend.get_board(bid)?;
                (cards, columns, board)
            }
            None => (self.list_live_cards_impl()?, Vec::new(), None),
        };
        let sprints = match (board.as_ref(), filter.search.as_deref()) {
            (Some(b), Some(q)) if !q.is_empty() => self.backend.list_sprints_by_board(b.id)?,
            _ => Vec::new(),
        };
        Ok(kanban_domain::filter_and_sort_cards(
            &cards,
            &columns,
            &sprints,
            board.as_ref(),
            filter,
        ))
    }
}
