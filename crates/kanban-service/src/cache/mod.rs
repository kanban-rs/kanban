use std::collections::HashSet;
use std::sync::Arc;

use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, DataStore, DependencyGraph, FetchPlan, FetchRound, FetchStatus,
    Invalidation, KanbanResult, LoadState, LoadedState, Resolved, Sprint,
};
use uuid::Uuid;

/// Entities are current as of their own individual fetch, not as of any one
/// pass over the cache. See [`EntityCache::resolve`].
#[derive(Debug, Default)]
pub struct EntityCache {
    boards: Collection<Board>,
    columns: Collection<Column>,
    cards: Collection<Card>,
    sprints: Collection<Sprint>,
    graph: LoadState<DependencyGraph>,
}

/// The two tiers of a `Collection` are independent: a loaded `all` never
/// back-fills `by_id`, and an entry in `by_id` never implies anything about
/// `all`. This matters concretely for cards: `DataStore::list_all_cards`
/// excludes archived cards while `DataStore::get_card` does not, so inferring
/// `by_id` from `all` would report an archived card as `Missing`.
///
/// `boards.by_id` stays permanently empty: `FetchRound` carries no per-board
/// id tier and `LoadedState` has no `board(id)` accessor, so nothing in this
/// cache can ever populate it.
struct LoadedView<'a>(&'a EntityCache);

impl LoadedState for LoadedView<'_> {
    fn board_list(&self) -> FetchStatus {
        (&self.0.boards.all).into()
    }
    fn column_list(&self) -> FetchStatus {
        (&self.0.columns.all).into()
    }
    fn card_list(&self) -> FetchStatus {
        (&self.0.cards.all).into()
    }
    fn sprint_list(&self) -> FetchStatus {
        (&self.0.sprints.all).into()
    }
    fn graph(&self) -> FetchStatus {
        (&self.0.graph).into()
    }
    fn column(&self, id: Uuid) -> FetchStatus {
        self.0
            .columns
            .by_id
            .get(&id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn card(&self, id: Uuid) -> FetchStatus {
        self.0
            .cards
            .by_id
            .get(&id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn sprint(&self, id: Uuid) -> FetchStatus {
        self.0
            .sprints
            .by_id
            .get(&id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
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
    }
}

fn outstanding<T>(ids: Vec<Uuid>, fetched: &HashSet<Uuid>, cached: &Collection<T>) -> Vec<Uuid> {
    ids.into_iter()
        .filter(|id| {
            !fetched.contains(id) && !matches!(cached.by_id.get(id), Some(LoadState::Missing))
        })
        .collect()
}

impl EntityCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn loaded_view(&self) -> LoadedView<'_> {
        LoadedView(self)
    }

    fn fetch_round(&mut self, round: &FetchRound, store: &dyn DataStore, resolved: &mut Resolved) {
        if round.board_list {
            let state = match store.list_boards() {
                Ok(v) => LoadState::Loaded(v),
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.boards.all = state.clone();
            resolved.boards.all = state;
        }
        if round.column_list {
            let state = match store.list_all_columns() {
                Ok(v) => LoadState::Loaded(v),
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.columns.all = state.clone();
            resolved.columns.all = state;
        }
        if round.card_list {
            let state = match store.list_all_cards() {
                Ok(v) => LoadState::Loaded(v),
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.cards.all = state.clone();
            resolved.cards.all = state;
        }
        if round.sprint_list {
            let state = match store.list_all_sprints() {
                Ok(v) => LoadState::Loaded(v),
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.sprints.all = state.clone();
            resolved.sprints.all = state;
        }
        if round.graph {
            let state = match store.get_graph() {
                Ok(g) => LoadState::Loaded(g),
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.graph = state.clone();
            resolved.graph = state;
        }

        for &id in &round.columns {
            let state = match store.get_column(id) {
                Ok(Some(v)) => LoadState::Loaded(v),
                Ok(None) => LoadState::Missing,
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.columns.by_id.insert(id, state.clone());
            resolved.columns.by_id.insert(id, state);
        }
        for &id in &round.cards {
            let state = match store.get_card(id) {
                Ok(Some(v)) => LoadState::Loaded(v),
                Ok(None) => LoadState::Missing,
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.cards.by_id.insert(id, state.clone());
            resolved.cards.by_id.insert(id, state);
        }
        for &id in &round.sprints {
            let state = match store.get_sprint(id) {
                Ok(Some(v)) => LoadState::Loaded(v),
                Ok(None) => LoadState::Missing,
                Err(e) => LoadState::Failed(Arc::new(e)),
            };
            self.sprints.by_id.insert(id, state.clone());
            resolved.sprints.by_id.insert(id, state);
        }
    }

    /// Drops anything this call has already fetched, plus any id cached as
    /// `LoadState::Missing`, which stays terminal in every scope. `Loaded`
    /// and `Failed` ids remain re-requestable by a later `resolve` call.
    fn narrow_to_outstanding(&self, round: FetchRound, fetched: &Fetched) -> FetchRound {
        FetchRound {
            board_list: round.board_list && !fetched.board_list,
            column_list: round.column_list && !fetched.column_list,
            card_list: round.card_list && !fetched.card_list,
            sprint_list: round.sprint_list && !fetched.sprint_list,
            graph: round.graph && !fetched.graph,
            columns: outstanding(round.columns, &fetched.columns, &self.columns),
            cards: outstanding(round.cards, &fetched.cards, &self.cards),
            sprints: outstanding(round.sprints, &fetched.sprints, &self.sprints),
        }
    }

    /// Loops until the plan has nothing left to ask for: each round is
    /// narrowed to what this call has not already fetched, and every fetch
    /// writes a terminal state, so the requestable set strictly shrinks and
    /// the loop halts structurally rather than on a round cap. A plan that
    /// keeps naming an entity it already received in this call is therefore
    /// harmless, and a need whose ids only become knowable after an earlier
    /// round resolves in the same call.
    ///
    /// Each entity reflects the backend as of its own individual fetch, not as
    /// of the resolve pass as a whole: two entities in the same `Resolved` may
    /// have been read moments apart. `resolve` deliberately does not wrap the
    /// rounds in one transaction, because that would hold a connection open
    /// across render-adjacent I/O today and across an unbounded number of
    /// rounds. The consequence of a torn read is a stale entity, never a
    /// fabricated one: every value returned came from a real backend
    /// response.
    pub fn resolve(
        &mut self,
        plan: &dyn FetchPlan,
        store: &dyn DataStore,
    ) -> KanbanResult<Resolved> {
        let mut resolved = Resolved::default();
        let mut fetched = Fetched::default();
        loop {
            let round = self.narrow_to_outstanding(plan.next_round(&self.loaded_view()), &fetched);
            if round.is_empty() {
                break;
            }
            self.fetch_round(&round, store, &mut resolved);
            fetched.record(&round);
        }
        Ok(resolved)
    }

    /// An `Entities` value naming no ids is treated as `All`: an
    /// invalidation that cannot say what changed must over-invalidate, never
    /// no-op, matching the polarity `invalidation_from_inverse` already uses
    /// for an unenumerable command batch.
    ///
    /// `prefixes` drops the board collection because this cache holds no
    /// prefix rows of its own, but a prefix write bumps a counter that
    /// board-derived display values read.
    pub fn invalidate(&mut self, invalidation: Invalidation) {
        let ids = match invalidation {
            Invalidation::All => {
                *self = Self::new();
                return;
            }
            Invalidation::Entities(ids) if ids.is_empty() => {
                *self = Self::new();
                return;
            }
            Invalidation::Entities(ids) => ids,
        };

        if !ids.boards.is_empty() || ids.prefixes {
            self.boards = Collection::default();
        }
        if !ids.columns.is_empty() {
            self.columns.all = LoadState::NotLoaded;
            for id in &ids.columns {
                self.columns.by_id.remove(id);
            }
        }
        if !ids.cards.is_empty() {
            self.cards.all = LoadState::NotLoaded;
            for id in &ids.cards {
                self.cards.by_id.remove(id);
            }
        }
        if !ids.sprints.is_empty() {
            self.sprints.all = LoadState::NotLoaded;
            for id in &ids.sprints {
                self.sprints.by_id.remove(id);
            }
        }
        if ids.graph {
            self.graph = LoadState::NotLoaded;
        }
    }

    pub fn board_list(&self) -> LoadState<&Vec<Board>> {
        self.boards.all.as_ref()
    }

    pub fn column_list(&self) -> LoadState<&Vec<Column>> {
        self.columns.all.as_ref()
    }

    pub fn card_list(&self) -> LoadState<&Vec<Card>> {
        self.cards.all.as_ref()
    }

    pub fn sprint_list(&self) -> LoadState<&Vec<Sprint>> {
        self.sprints.all.as_ref()
    }

    pub fn graph(&self) -> LoadState<&DependencyGraph> {
        self.graph.as_ref()
    }

    pub fn column(&self, id: Uuid) -> LoadState<&Column> {
        self.columns
            .by_id
            .get(&id)
            .map(LoadState::as_ref)
            .unwrap_or(LoadState::NotLoaded)
    }

    pub fn card(&self, id: Uuid) -> LoadState<&Card> {
        self.cards
            .by_id
            .get(&id)
            .map(LoadState::as_ref)
            .unwrap_or(LoadState::NotLoaded)
    }

    pub fn sprint(&self, id: Uuid) -> LoadState<&Sprint> {
        self.sprints
            .by_id
            .get(&id)
            .map(LoadState::as_ref)
            .unwrap_or(LoadState::NotLoaded)
    }
}

#[cfg(test)]
mod tests;
