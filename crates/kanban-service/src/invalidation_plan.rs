use kanban_domain::{EntityIds, Invalidation};

use crate::fetch_plan::{
    requestable, FetchPlan, FetchRound, FetchStatus, LoadedEntities, LoadedState,
};

/// A [`FetchPlan`] built from an [`Invalidation`] plus a pre-invalidation
/// [`LoadedState`] snapshot. Construct it *before* calling
/// [`kanban_domain::Model::invalidate`]: it re-requests exactly the tiers
/// `invalidate` is about to blank, restricted to tiers the snapshot shows
/// were already read, so it never asks for something the caller never
/// wanted in the first place.
#[derive(Debug, Clone)]
pub struct InvalidationPlan {
    round: FetchRound,
}

/// `NotLoaded` is precisely "nobody ever asked". `Loaded`, `Failed` and
/// `Missing` all mean a fetch was attempted, and `Model::invalidate` clears a
/// cached `Missing` by removing the entry, so all three count as "was read".
fn was_read(status: FetchStatus) -> bool {
    !matches!(status, FetchStatus::NotLoaded)
}

fn already_read(
    ids: &std::collections::HashSet<uuid::Uuid>,
    status_of: impl Fn(uuid::Uuid) -> FetchStatus,
) -> Vec<uuid::Uuid> {
    let mut ids: Vec<uuid::Uuid> = ids
        .iter()
        .copied()
        .filter(|id| was_read(status_of(*id)))
        .collect();
    ids.sort_unstable();
    ids
}

fn outstanding(
    ids: &[uuid::Uuid],
    status_of: impl Fn(uuid::Uuid) -> FetchStatus,
) -> Vec<uuid::Uuid> {
    ids.iter()
        .copied()
        .filter(|id| requestable(status_of(*id)))
        .collect()
}

impl InvalidationPlan {
    /// `loaded` must be the `Model`'s state *before* `Model::invalidate` is
    /// applied. Never substitute an empty plan for `None`: `None` means
    /// nothing invalidated needs re-fetching, while `Some` with an
    /// all-`false`/all-empty round would be a distinct, wrong signal to
    /// callers that check `is_none()` to skip a refetch cycle.
    pub fn for_invalidation(invalidation: &Invalidation, loaded: &dyn LoadedState) -> Option<Self> {
        let round = match invalidation {
            Invalidation::All => FetchRound {
                board_list: was_read(loaded.board_list()),
                column_list: was_read(loaded.column_list()),
                card_list: was_read(loaded.card_list()),
                sprint_list: was_read(loaded.sprint_list()),
                graph: was_read(loaded.graph()),
                columns: Vec::new(),
                cards: Vec::new(),
                sprints: Vec::new(),
                columns_by_board: Vec::new(),
                cards_by_column: Vec::new(),
                sprints_by_board: Vec::new(),
                archived_card_list: was_read(loaded.archived_card_list()),
                archived_cards_by_board: Vec::new(),
                archived_board_list: was_read(loaded.archived_board_list()),
            },
            Invalidation::Entities(ids) if ids.is_empty() => return None,
            Invalidation::Entities(ids) => build_entities_round(ids, loaded),
        };

        (!round.is_empty()).then_some(Self { round })
    }

    pub fn round(&self) -> &FetchRound {
        &self.round
    }
}

fn build_entities_round(ids: &EntityIds, loaded: &dyn LoadedState) -> FetchRound {
    FetchRound {
        board_list: (!ids.boards.is_empty() || ids.prefixes) && was_read(loaded.board_list()),
        column_list: !ids.columns.is_empty() && was_read(loaded.column_list()),
        card_list: !ids.cards.is_empty() && was_read(loaded.card_list()),
        sprint_list: !ids.sprints.is_empty() && was_read(loaded.sprint_list()),
        graph: ids.graph && was_read(loaded.graph()),
        columns: already_read(&ids.columns, |id| loaded.column(id)),
        cards: already_read(&ids.cards, |id| loaded.card(id)),
        sprints: already_read(&ids.sprints, |id| loaded.sprint(id)),
        columns_by_board: Vec::new(),
        cards_by_column: Vec::new(),
        sprints_by_board: Vec::new(),
        // `Model::invalidate`'s `Entities` path never blanks either flat
        // archival tier; only `load_from_snapshot` recomputes them.
        archived_card_list: false,
        archived_cards_by_board: Vec::new(),
        archived_board_list: false,
    }
}

impl FetchPlan for InvalidationPlan {
    /// Both `FetchRound` literals in this file are exhaustive, deliberately
    /// against the usual plan convention. A tier `Model::invalidate` blanks
    /// and this plan silently defaults to "not requested" is exactly the
    /// defect this type exists to close, so a new tier must break the build
    /// here. Never repair such a break with `..Default::default()`; that
    /// trades the compile error for the silent gap. Add the field by name
    /// and decide it.
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            board_list: self.round.board_list && requestable(loaded.board_list()),
            column_list: self.round.column_list && requestable(loaded.column_list()),
            card_list: self.round.card_list && requestable(loaded.card_list()),
            sprint_list: self.round.sprint_list && requestable(loaded.sprint_list()),
            graph: self.round.graph && requestable(loaded.graph()),
            columns: outstanding(&self.round.columns, |id| loaded.column(id)),
            cards: outstanding(&self.round.cards, |id| loaded.card(id)),
            sprints: outstanding(&self.round.sprints, |id| loaded.sprint(id)),
            columns_by_board: Vec::new(),
            cards_by_column: Vec::new(),
            sprints_by_board: Vec::new(),
            archived_card_list: self.round.archived_card_list
                && requestable(loaded.archived_card_list()),
            archived_cards_by_board: Vec::new(),
            archived_board_list: self.round.archived_board_list
                && requestable(loaded.archived_board_list()),
        }
    }
}

#[cfg(test)]
mod tests;
