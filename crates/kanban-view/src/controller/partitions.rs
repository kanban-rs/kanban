use super::Controller;
use kanban_domain::{filter_and_sort_boards, Board, BoardListFilter, Card, Model};

impl Controller {
    pub(super) fn rebuild_card_partitions(&mut self, model: &Model) {
        let (archived_cards, live_cards): (Vec<Card>, Vec<Card>) = model
            .cards_state()
            .loaded_or_empty()
            .iter()
            .cloned()
            .partition(|c| model.archived_card_ids().contains(&c.id));
        self.displayed_cards_live = live_cards;
        self.displayed_cards_archived = archived_cards;
    }

    pub(super) fn rebuild_board_partitions(&mut self, model: &Model) {
        let (archived_boards, live_boards): (Vec<Board>, Vec<Board>) = model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .cloned()
            .partition(|b| model.archived_board_ids().contains(&b.id));
        self.displayed_boards_live = live_boards;
        self.displayed_boards_archived = archived_boards;
        self.sort_partitions();
    }

    /// Sort BOTH cached board partitions, each against its own independent
    /// field/order pair. Called on sync and whenever either sort dimension
    /// changes, so the rendered lists and the selection resolvers (which read
    /// these partitions) stay consistent.
    pub(super) fn sort_partitions(&mut self) {
        let live_filter = BoardListFilter {
            sort: Some(self.live_board_sort_field),
            sort_order: Some(self.live_board_sort_order),
            ..Default::default()
        };
        self.displayed_boards_live = filter_and_sort_boards(
            &self.displayed_boards_live,
            &live_filter,
            &self.archived_board_at,
            None,
        );
        let archived_filter = BoardListFilter {
            sort: Some(self.archived_board_sort_field),
            sort_order: Some(self.archived_board_sort_order),
            ..Default::default()
        };
        self.displayed_boards_archived = filter_and_sort_boards(
            &self.displayed_boards_archived,
            &archived_filter,
            &self.archived_board_at,
            None,
        );
    }
}
