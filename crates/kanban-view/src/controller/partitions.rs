use super::Controller;
use kanban_domain::{filter_and_sort_boards, Board, BoardListFilter, Card, LoadState, Model};

/// The join of a flat collection's state and its archival-marker tier's
/// state, payload blanked: `Failed` wins, then `Missing`, then `NotLoaded`.
/// `Loaded` only when both sides are.
fn joined<T, A, B>(flat: LoadState<A>, markers: LoadState<B>) -> LoadState<Vec<T>> {
    match (flat, markers) {
        (LoadState::Failed(e), _) | (_, LoadState::Failed(e)) => LoadState::Failed(e),
        (LoadState::Missing, _) | (_, LoadState::Missing) => LoadState::Missing,
        (LoadState::NotLoaded, _) | (_, LoadState::NotLoaded) => LoadState::NotLoaded,
        (LoadState::Loaded(_), LoadState::Loaded(_)) => LoadState::Loaded(Vec::new()),
    }
}

/// Joins a board's archived-marker tier with the per-id card body tier: for
/// each Loaded marker, resolves its body through `card_by_id_state`. A
/// `Missing` body is skipped (a stale marker never wedges the view); a
/// `Failed` body wins over the whole join regardless of where in the marker
/// order it falls; a `NotLoaded` body means the join is `NotLoaded`, unless a
/// later marker in the same walk turns out `Failed`.
fn archived_bodies(model: &Model, board_id: uuid::Uuid) -> LoadState<Vec<Card>> {
    let markers = match model.board_archived_cards_state(board_id) {
        LoadState::Loaded(markers) => markers,
        LoadState::Failed(e) => return LoadState::Failed(e),
        LoadState::Missing => return LoadState::Missing,
        LoadState::NotLoaded => return LoadState::NotLoaded,
    };

    let mut cards = Vec::with_capacity(markers.len());
    let mut not_loaded = false;
    for marker in markers {
        match model.card_by_id_state(marker.entity_id) {
            LoadState::Loaded(card) => cards.push(card.clone()),
            LoadState::Missing => continue,
            LoadState::Failed(e) => return LoadState::Failed(e),
            LoadState::NotLoaded => not_loaded = true,
        }
    }
    if not_loaded {
        return LoadState::NotLoaded;
    }
    LoadState::Loaded(cards)
}

impl Controller {
    pub(super) fn rebuild_card_partitions(&mut self, model: &Model) {
        let Some(board_id) = self.scope_board else {
            self.displayed_cards_live = LoadState::NotLoaded;
            self.displayed_cards_archived = LoadState::NotLoaded;
            return;
        };

        self.displayed_cards_live = model
            .board_cards_state(board_id)
            .map(|cards| cards.into_iter().cloned().collect());
        self.displayed_cards_archived = archived_bodies(model, board_id);
    }

    pub(super) fn rebuild_board_partitions(&mut self, model: &Model) {
        match (model.boards_state().as_ref(), model.archived_boards_state()) {
            (LoadState::Loaded(boards), LoadState::Loaded(_)) => {
                let (archived_boards, live_boards): (Vec<Board>, Vec<Board>) = boards
                    .iter()
                    .cloned()
                    .partition(|b| model.archived_board_ids().contains(&b.id));
                self.displayed_boards_live = LoadState::Loaded(live_boards);
                self.displayed_boards_archived = LoadState::Loaded(archived_boards);
            }
            (LoadState::Loaded(boards), markers) => {
                self.displayed_boards_live = LoadState::Loaded(boards.clone());
                self.displayed_boards_archived = markers.map(|_| Vec::new());
            }
            (flat, markers) => {
                self.displayed_boards_live = flat.clone().map(|_| Vec::new());
                self.displayed_boards_archived = joined(flat, markers);
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
