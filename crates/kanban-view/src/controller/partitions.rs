use super::Controller;
use kanban_domain::{filter_and_sort_boards, Board, BoardListFilter, Card, LoadState, Model};

impl Controller {
    pub(super) fn rebuild_card_partitions(&mut self, model: &Model) {
        match model.cards_state().as_ref() {
            LoadState::Loaded(cards) => {
                let (archived_cards, live_cards): (Vec<Card>, Vec<Card>) = cards
                    .iter()
                    .cloned()
                    .partition(|c| model.archived_card_ids().contains(&c.id));
                self.displayed_cards_live = LoadState::Loaded(live_cards);
                self.displayed_cards_archived = LoadState::Loaded(archived_cards);
            }
            other => {
                self.displayed_cards_live = other.as_ref().map(|_| Vec::new());
                self.displayed_cards_archived = other.map(|_| Vec::new());
            }
        }
    }

    pub(super) fn rebuild_board_partitions(&mut self, model: &Model) {
        match model.boards_state().as_ref() {
            LoadState::Loaded(boards) => {
                let (archived_boards, live_boards): (Vec<Board>, Vec<Board>) = boards
                    .iter()
                    .cloned()
                    .partition(|b| model.archived_board_ids().contains(&b.id));
                self.displayed_boards_live = LoadState::Loaded(live_boards);
                self.displayed_boards_archived = LoadState::Loaded(archived_boards);
            }
            other => {
                self.displayed_boards_live = other.as_ref().map(|_| Vec::new());
                self.displayed_boards_archived = other.map(|_| Vec::new());
            }
        }
        self.sort_partitions();
    }

    /// Sort BOTH cached board partitions, each against its own independent
    /// field/order pair. Called on sync and whenever either sort dimension
    /// changes, so the rendered lists and the selection resolvers (which read
    /// these partitions) stay consistent. Only a `Loaded` partition is sorted;
    /// any other state is left as-is.
    pub(super) fn sort_partitions(&mut self) {
        if let LoadState::Loaded(boards) = &self.displayed_boards_live {
            let live_filter = BoardListFilter {
                sort: Some(self.live_board_sort_field),
                sort_order: Some(self.live_board_sort_order),
                ..Default::default()
            };
            let sorted =
                filter_and_sort_boards(boards, &live_filter, &self.archived_board_at, None);
            self.displayed_boards_live = LoadState::Loaded(sorted);
        }
        if let LoadState::Loaded(boards) = &self.displayed_boards_archived {
            let archived_filter = BoardListFilter {
                sort: Some(self.archived_board_sort_field),
                sort_order: Some(self.archived_board_sort_order),
                ..Default::default()
            };
            let sorted =
                filter_and_sort_boards(boards, &archived_filter, &self.archived_board_at, None);
            self.displayed_boards_archived = LoadState::Loaded(sorted);
        }
    }
}
