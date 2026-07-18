use super::KanbanContext;
use kanban_domain::{Card, CardListFilter, KanbanResult};

impl KanbanContext {
    pub(super) fn filter_cards(&self, filter: &CardListFilter) -> KanbanResult<Vec<Card>> {
        let cards = self.list_live_cards_impl()?;
        let board = match filter.board_id {
            Some(bid) => self.backend.get_board(bid)?,
            None => None,
        };
        let columns = match filter.board_id {
            Some(bid) => self.backend.list_columns_by_board(bid)?,
            None => Vec::new(),
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
