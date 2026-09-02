use std::collections::HashSet;
use std::sync::Arc;

use kanban_domain::{DataStore, LoadState, Resolved};
use uuid::Uuid;

use crate::fetch_plan::{FetchPlan, FetchRound, FetchStatus, LoadedEntities, LoadedState};

pub(crate) struct Overlay<'a> {
    pub(crate) base: &'a dyn LoadedEntities,
    pub(crate) pass: &'a Resolved,
}

fn overlay_status<T>(pass: &LoadState<T>, base: FetchStatus) -> FetchStatus {
    match pass {
        LoadState::NotLoaded => base,
        other => other.into(),
    }
}

impl LoadedState for Overlay<'_> {
    fn board_list(&self) -> FetchStatus {
        overlay_status(&self.pass.boards.all, self.base.board_list())
    }
    fn column_list(&self) -> FetchStatus {
        overlay_status(&self.pass.columns.all, self.base.column_list())
    }
    fn card_list(&self) -> FetchStatus {
        overlay_status(&self.pass.cards.all, self.base.card_list())
    }
    fn sprint_list(&self) -> FetchStatus {
        overlay_status(&self.pass.sprints.all, self.base.sprint_list())
    }
    fn graph(&self) -> FetchStatus {
        overlay_status(&self.pass.graph, self.base.graph())
    }
    fn column(&self, id: Uuid) -> FetchStatus {
        match self.pass.columns.by_id.get(&id) {
            Some(state) => state.into(),
            None => self.base.column(id),
        }
    }
    fn card(&self, id: Uuid) -> FetchStatus {
        match self.pass.cards.by_id.get(&id) {
            Some(state) => state.into(),
            None => self.base.card(id),
        }
    }
    fn sprint(&self, id: Uuid) -> FetchStatus {
        match self.pass.sprints.by_id.get(&id) {
            Some(state) => state.into(),
            None => self.base.sprint(id),
        }
    }
    fn columns_of_board(&self, board_id: Uuid) -> FetchStatus {
        match self.pass.columns.by_parent.get(&board_id) {
            Some(state) => state.into(),
            None => self.base.columns_of_board(board_id),
        }
    }
    fn cards_of_column(&self, column_id: Uuid) -> FetchStatus {
        match self.pass.cards.by_parent.get(&column_id) {
            Some(state) => state.into(),
            None => self.base.cards_of_column(column_id),
        }
    }
    fn sprints_of_board(&self, board_id: Uuid) -> FetchStatus {
        match self.pass.sprints.by_parent.get(&board_id) {
            Some(state) => state.into(),
            None => self.base.sprints_of_board(board_id),
        }
    }
    fn archived_card_list(&self) -> FetchStatus {
        overlay_status(
            &self.pass.archived_cards.all,
            self.base.archived_card_list(),
        )
    }
    fn archived_cards_of_board(&self, board_id: Uuid) -> FetchStatus {
        match self.pass.archived_cards.by_parent.get(&board_id) {
            Some(state) => state.into(),
            None => self.base.archived_cards_of_board(board_id),
        }
    }
    fn archived_board_list(&self) -> FetchStatus {
        overlay_status(
            &self.pass.archived_boards.all,
            self.base.archived_board_list(),
        )
    }
}

impl LoadedEntities for Overlay<'_> {
    fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[kanban_domain::Column]> {
        match self.pass.columns.by_parent.get(&board_id) {
            Some(LoadState::Loaded(v)) => Some(v.as_slice()),
            Some(_) => None,
            None => self.base.loaded_columns_of_board(board_id),
        }
    }
}

#[derive(Default)]
struct Fetched {
    board_list: bool,
    column_list: bool,
    card_list: bool,
    sprint_list: bool,
    graph: bool,
    columns: HashSet<Uuid>,
    cards: HashSet<Uuid>,
    sprints: HashSet<Uuid>,
    columns_by_board: HashSet<Uuid>,
    cards_by_column: HashSet<Uuid>,
    sprints_by_board: HashSet<Uuid>,
    archived_card_list: bool,
    archived_cards_by_board: HashSet<Uuid>,
    archived_board_list: bool,
}

impl Fetched {
    fn record(&mut self, round: &FetchRound) {
        self.board_list |= round.board_list;
        self.column_list |= round.column_list;
        self.card_list |= round.card_list;
        self.sprint_list |= round.sprint_list;
        self.graph |= round.graph;
        self.columns.extend(round.columns.iter().copied());
        self.cards.extend(round.cards.iter().copied());
        self.sprints.extend(round.sprints.iter().copied());
        self.columns_by_board
            .extend(round.columns_by_board.iter().copied());
        self.cards_by_column
            .extend(round.cards_by_column.iter().copied());
        self.sprints_by_board
            .extend(round.sprints_by_board.iter().copied());
        self.archived_card_list |= round.archived_card_list;
        self.archived_cards_by_board
            .extend(round.archived_cards_by_board.iter().copied());
        self.archived_board_list |= round.archived_board_list;
    }
}

fn outstanding(
    ids: Vec<Uuid>,
    fetched: &HashSet<Uuid>,
    status: impl Fn(Uuid) -> FetchStatus,
) -> Vec<Uuid> {
    ids.into_iter()
        .filter(|&id| !fetched.contains(&id) && status(id) != FetchStatus::Missing)
        .collect()
}

fn outstanding_scoped(parents: Vec<Uuid>, fetched: &HashSet<Uuid>) -> Vec<Uuid> {
    parents
        .into_iter()
        .filter(|id| !fetched.contains(id))
        .collect()
}

fn narrow_to_outstanding(
    round: FetchRound,
    fetched: &Fetched,
    loaded: &dyn LoadedState,
) -> FetchRound {
    FetchRound {
        board_list: round.board_list && !fetched.board_list,
        column_list: round.column_list && !fetched.column_list,
        card_list: round.card_list && !fetched.card_list,
        sprint_list: round.sprint_list && !fetched.sprint_list,
        graph: round.graph && !fetched.graph,
        columns: outstanding(round.columns, &fetched.columns, |id| loaded.column(id)),
        cards: outstanding(round.cards, &fetched.cards, |id| loaded.card(id)),
        sprints: outstanding(round.sprints, &fetched.sprints, |id| loaded.sprint(id)),
        columns_by_board: outstanding_scoped(round.columns_by_board, &fetched.columns_by_board),
        cards_by_column: outstanding_scoped(round.cards_by_column, &fetched.cards_by_column),
        sprints_by_board: outstanding_scoped(round.sprints_by_board, &fetched.sprints_by_board),
        archived_card_list: round.archived_card_list && !fetched.archived_card_list,
        archived_cards_by_board: outstanding_scoped(
            round.archived_cards_by_board,
            &fetched.archived_cards_by_board,
        ),
        archived_board_list: round.archived_board_list && !fetched.archived_board_list,
    }
}

fn fetch_round(round: &FetchRound, store: &dyn DataStore, resolved: &mut Resolved) {
    if round.board_list {
        resolved.boards.all = match store.list_boards() {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }
    if round.column_list {
        resolved.columns.all = match store.list_all_columns() {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }
    if round.card_list {
        resolved.cards.all = match store.list_all_cards() {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }
    if round.sprint_list {
        resolved.sprints.all = match store.list_all_sprints() {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }
    if round.graph {
        resolved.graph = match store.get_graph() {
            Ok(g) => LoadState::Loaded(g),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }
    if round.archived_card_list {
        resolved.archived_cards.all = match store.list_archived_cards() {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }
    if round.archived_board_list {
        resolved.archived_boards.all = match store.list_archived_boards() {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
    }

    for &id in &round.columns {
        let state = match store.get_column(id) {
            Ok(Some(v)) => LoadState::Loaded(v),
            Ok(None) => LoadState::Missing,
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.columns.by_id.insert(id, state);
    }
    for &id in &round.cards {
        let state = match store.get_card(id) {
            Ok(Some(v)) => LoadState::Loaded(v),
            Ok(None) => LoadState::Missing,
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.cards.by_id.insert(id, state);
    }
    for &id in &round.sprints {
        let state = match store.get_sprint(id) {
            Ok(Some(v)) => LoadState::Loaded(v),
            Ok(None) => LoadState::Missing,
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.sprints.by_id.insert(id, state);
    }

    for &board_id in &round.columns_by_board {
        let state = match store.list_columns_by_board(board_id) {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.columns.by_parent.insert(board_id, state);
    }
    for &column_id in &round.cards_by_column {
        let state = match store.list_cards_by_column(column_id) {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.cards.by_parent.insert(column_id, state);
    }
    for &board_id in &round.sprints_by_board {
        let state = match store.list_sprints_by_board(board_id) {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.sprints.by_parent.insert(board_id, state);
    }
    for &board_id in &round.archived_cards_by_board {
        let state = match store.list_archived_cards_by_board(board_id) {
            Ok(v) => LoadState::Loaded(v),
            Err(e) => LoadState::Failed(Arc::new(e)),
        };
        resolved.archived_cards.by_parent.insert(board_id, state);
    }
}

/// Loops until the plan has nothing left to ask for: each round is
/// narrowed to what this call has not already fetched, and every fetch
/// writes a terminal state, so the requestable set strictly shrinks and
/// the loop halts structurally rather than on a round cap. A plan that
/// keeps naming an entity it already received in this call is therefore
/// harmless, and a need whose ids only become knowable after an earlier
/// round resolves in the same call, because the plan is consulted through
/// an `Overlay` of the caller's `loaded` and the in-flight `Resolved`.
///
/// Each entity reflects the backend as of its own individual fetch, not as
/// of the resolve pass as a whole: two entities in the same `Resolved` may
/// have been read moments apart. `resolve` deliberately does not wrap the
/// rounds in one transaction, because that would hold a connection open
/// across render-adjacent I/O today and across an unbounded number of
/// rounds. The consequence of a torn read is a stale entity, never a
/// fabricated one: every value returned came from a real backend
/// response.
///
/// A parent-scoped fetch has no terminal-`Missing` shortcut: the scoped
/// `DataStore` reads answer an unknown parent with an empty vector, so
/// [`outstanding_scoped`] narrows only against what this call has already
/// fetched, never against `loaded`, which keeps a scope refetchable by a
/// later call the same way the whole-list tier already is.
pub fn resolve(
    plan: &dyn FetchPlan,
    loaded: &dyn LoadedEntities,
    store: &dyn DataStore,
) -> Resolved {
    let mut resolved = Resolved::default();
    let mut fetched = Fetched::default();
    loop {
        let round = {
            let overlay = Overlay {
                base: loaded,
                pass: &resolved,
            };
            narrow_to_outstanding(plan.next_round(&overlay), &fetched, &overlay)
        };
        if round.is_empty() {
            break;
        }
        fetch_round(&round, store, &mut resolved);
        fetched.record(&round);
    }
    resolved
}

#[cfg(test)]
mod tests;
