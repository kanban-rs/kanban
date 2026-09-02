#![allow(dead_code)]

use std::cell::{Cell, RefCell};

use kanban_domain::resolved::Collection;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, LoadState, Sprint,
};
use uuid::Uuid;

use crate::fetch_plan::{
    requestable, FetchPlan, FetchRound, FetchStatus, LoadedEntities,
    LoadedState as LoadedStateTrait,
};
use crate::read_recorder::RecordingStore;

mod archived;
mod archived_board;
mod multi_round;
mod overlay;
mod resolve;
mod result_mapping;
mod retry;
mod scoped;

#[derive(Default)]
pub(super) struct StubLoaded {
    boards: Collection<Board>,
    columns: Collection<Column>,
    cards: Collection<Card>,
    sprints: Collection<Sprint>,
    archived_cards: Collection<ArchivedCard>,
    archived_boards: Collection<ArchivedBoard>,
    graph: LoadState<DependencyGraph>,
}

impl LoadedStateTrait for StubLoaded {
    fn board_list(&self) -> FetchStatus {
        (&self.boards.all).into()
    }
    fn column_list(&self) -> FetchStatus {
        (&self.columns.all).into()
    }
    fn card_list(&self) -> FetchStatus {
        (&self.cards.all).into()
    }
    fn sprint_list(&self) -> FetchStatus {
        (&self.sprints.all).into()
    }
    fn graph(&self) -> FetchStatus {
        (&self.graph).into()
    }
    fn column(&self, id: Uuid) -> FetchStatus {
        self.columns
            .by_id
            .get(&id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn card(&self, id: Uuid) -> FetchStatus {
        self.cards
            .by_id
            .get(&id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn sprint(&self, id: Uuid) -> FetchStatus {
        self.sprints
            .by_id
            .get(&id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn columns_of_board(&self, board_id: Uuid) -> FetchStatus {
        self.columns
            .by_parent
            .get(&board_id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn cards_of_column(&self, column_id: Uuid) -> FetchStatus {
        self.cards
            .by_parent
            .get(&column_id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn sprints_of_board(&self, board_id: Uuid) -> FetchStatus {
        self.sprints
            .by_parent
            .get(&board_id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn archived_card_list(&self) -> FetchStatus {
        (&self.archived_cards.all).into()
    }
    fn archived_cards_of_board(&self, board_id: Uuid) -> FetchStatus {
        self.archived_cards
            .by_parent
            .get(&board_id)
            .map(FetchStatus::from)
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn archived_board_list(&self) -> FetchStatus {
        (&self.archived_boards.all).into()
    }
}

impl LoadedEntities for StubLoaded {
    fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]> {
        self.columns
            .by_parent
            .get(&board_id)
            .and_then(LoadState::loaded)
            .map(Vec::as_slice)
    }
}

fn apply_collection<T: Clone>(target: &mut Collection<T>, incoming: Collection<T>) {
    if !matches!(incoming.all, LoadState::NotLoaded) {
        target.all = incoming.all;
    }
    for (id, state) in incoming.by_id {
        target.by_id.insert(id, state);
    }
    for (key, state) in incoming.by_parent {
        target.by_parent.insert(key, state);
    }
}

impl StubLoaded {
    pub(super) fn apply(&mut self, resolved: kanban_domain::Resolved) {
        apply_collection(&mut self.boards, resolved.boards);
        apply_collection(&mut self.columns, resolved.columns);
        apply_collection(&mut self.cards, resolved.cards);
        apply_collection(&mut self.sprints, resolved.sprints);
        apply_collection(&mut self.archived_cards, resolved.archived_cards);
        apply_collection(&mut self.archived_boards, resolved.archived_boards);
        if !matches!(resolved.graph, LoadState::NotLoaded) {
            self.graph = resolved.graph;
        }
    }

    pub(super) fn forget_card(&mut self, id: Uuid) {
        self.cards.by_id.remove(&id);
    }

    pub(super) fn board_list_state(&self) -> LoadState<&Vec<Board>> {
        self.boards.all.as_ref()
    }

    pub(super) fn column_list_state(&self) -> LoadState<&Vec<Column>> {
        self.columns.all.as_ref()
    }

    pub(super) fn card_list_state(&self) -> LoadState<&Vec<Card>> {
        self.cards.all.as_ref()
    }

    pub(super) fn sprint_list_state(&self) -> LoadState<&Vec<Sprint>> {
        self.sprints.all.as_ref()
    }

    pub(super) fn graph_state(&self) -> LoadState<&DependencyGraph> {
        self.graph.as_ref()
    }

    pub(super) fn column_state(&self, id: Uuid) -> LoadState<&Column> {
        self.columns
            .by_id
            .get(&id)
            .map(LoadState::as_ref)
            .unwrap_or(LoadState::NotLoaded)
    }

    pub(super) fn card_state(&self, id: Uuid) -> LoadState<&Card> {
        self.cards
            .by_id
            .get(&id)
            .map(LoadState::as_ref)
            .unwrap_or(LoadState::NotLoaded)
    }

    pub(super) fn sprint_state(&self, id: Uuid) -> LoadState<&Sprint> {
        self.sprints
            .by_id
            .get(&id)
            .map(LoadState::as_ref)
            .unwrap_or(LoadState::NotLoaded)
    }
}

pub(super) fn store() -> RecordingStore {
    RecordingStore::new()
}

pub(super) fn seed_board(store: &RecordingStore, name: &str) -> Board {
    let board = Board::new(name, None::<String>);
    store.upsert_board(board.clone()).unwrap();
    board
}

pub(super) fn seed_column(store: &RecordingStore, board: &Board, name: &str) -> Column {
    let column = Column::new(board.id, name, 0);
    store.upsert_column(column.clone()).unwrap();
    column
}

pub(super) fn seed_card(
    store: &RecordingStore,
    board: &Board,
    column: &Column,
    title: &str,
) -> Card {
    let card = Card::new(board.id, column.id, title, 0);
    store.upsert_card(card.clone()).unwrap();
    card
}

pub(super) fn seed_sprint(store: &RecordingStore, board: &Board) -> Sprint {
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    store.upsert_sprint(sprint.clone()).unwrap();
    sprint
}

pub(super) fn seed_archived_card(
    store: &RecordingStore,
    board: &Board,
    card: &Card,
) -> ArchivedCard {
    let marker = ArchivedCard::new(card.id, board.id);
    store.insert_archived_card(marker).unwrap();
    marker
}

pub(super) fn seed_archived_board(store: &RecordingStore, board: &Board) -> ArchivedBoard {
    let marker = ArchivedBoard::now(board.id);
    store.insert_archived_board(marker).unwrap();
    marker
}

pub(super) fn seed_board_with_column(store: &RecordingStore) -> (Board, Column) {
    let board = seed_board(store, "b");
    let column = seed_column(store, &board, "c");
    (board, column)
}

pub(super) struct FixedPlan(pub FetchRound);

impl FetchPlan for FixedPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        self.0.clone()
    }
}

pub(super) struct OneShotPlan(pub FetchRound, pub Cell<bool>);

impl OneShotPlan {
    pub(super) fn new(round: FetchRound) -> Self {
        Self(round, Cell::new(false))
    }
}

impl FetchPlan for OneShotPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        if self.1.get() {
            FetchRound::default()
        } else {
            self.1.set(true);
            self.0.clone()
        }
    }
}

pub(super) struct BoardListPlan;

impl FetchPlan for BoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            board_list: requestable(loaded.board_list()),
            ..Default::default()
        }
    }
}

pub(super) struct GraphThenCardPlan {
    pub card_id: Uuid,
}

impl FetchPlan for GraphThenCardPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        if requestable(loaded.graph()) {
            FetchRound {
                graph: true,
                ..Default::default()
            }
        } else if requestable(loaded.card(self.card_id)) {
            FetchRound {
                cards: vec![self.card_id],
                ..Default::default()
            }
        } else {
            FetchRound::default()
        }
    }
}

pub(super) struct ChainPlan {
    pub ids: Vec<Uuid>,
}

impl FetchPlan for ChainPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        match self
            .ids
            .iter()
            .find(|&&id| loaded.card(id) != FetchStatus::Loaded)
        {
            Some(&id) => FetchRound {
                cards: vec![id],
                ..Default::default()
            },
            None => FetchRound::default(),
        }
    }
}

pub(super) struct StickyThenNextPlan {
    pub first: Uuid,
    pub second: Uuid,
}

impl FetchPlan for StickyThenNextPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        let mut cards = vec![self.first];
        if loaded.card(self.first) == FetchStatus::Loaded && requestable(loaded.card(self.second)) {
            cards.push(self.second);
        }
        FetchRound {
            cards,
            ..Default::default()
        }
    }
}

pub(super) struct CardListThenCardPlan {
    pub card_id: Uuid,
}

impl FetchPlan for CardListThenCardPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        if requestable(loaded.card_list()) {
            FetchRound {
                card_list: true,
                ..Default::default()
            }
        } else if requestable(loaded.card(self.card_id)) {
            FetchRound {
                cards: vec![self.card_id],
                ..Default::default()
            }
        } else {
            FetchRound::default()
        }
    }
}

pub(super) struct HierarchyPlan {
    pub board_id: Uuid,
}

impl FetchPlan for HierarchyPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        if requestable(loaded.board_list()) {
            return FetchRound {
                board_list: true,
                ..Default::default()
            };
        }
        if requestable(loaded.columns_of_board(self.board_id)) {
            return FetchRound {
                columns_by_board: vec![self.board_id],
                ..Default::default()
            };
        }
        match loaded.loaded_columns_of_board(self.board_id) {
            Some(columns) => {
                let ids: Vec<Uuid> = columns
                    .iter()
                    .map(|c| c.id)
                    .filter(|&id| requestable(loaded.cards_of_column(id)))
                    .collect();
                FetchRound {
                    cards_by_column: ids,
                    ..Default::default()
                }
            }
            None => FetchRound::default(),
        }
    }
}

pub(super) struct CardsByIdPlan {
    pub ids: Vec<Uuid>,
}

impl FetchPlan for CardsByIdPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            cards: self
                .ids
                .iter()
                .copied()
                .filter(|&id| requestable(loaded.card(id)))
                .collect(),
            ..Default::default()
        }
    }
}

pub(super) struct ArchivedByBoardPlan {
    pub board_id: Uuid,
}

impl FetchPlan for ArchivedByBoardPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        if requestable(loaded.archived_cards_of_board(self.board_id)) {
            FetchRound {
                archived_cards_by_board: vec![self.board_id],
                ..Default::default()
            }
        } else {
            FetchRound::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Observed {
    pub board_list: FetchStatus,
    pub column_list: FetchStatus,
    pub card_list: FetchStatus,
    pub sprint_list: FetchStatus,
    pub graph: FetchStatus,
    pub column: FetchStatus,
    pub card: FetchStatus,
    pub sprint: FetchStatus,
    pub columns_of_board: FetchStatus,
    pub cards_of_column: FetchStatus,
    pub sprints_of_board: FetchStatus,
}

pub(super) struct ProbePlan {
    pub round: FetchRound,
    pub board_id: Uuid,
    pub column_id: Uuid,
    pub card_id: Uuid,
    pub sprint_id: Uuid,
    pub seen: RefCell<Vec<Observed>>,
}

impl ProbePlan {
    pub(super) fn new(
        round: FetchRound,
        board_id: Uuid,
        column_id: Uuid,
        card_id: Uuid,
        sprint_id: Uuid,
    ) -> Self {
        Self {
            round,
            board_id,
            column_id,
            card_id,
            sprint_id,
            seen: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn last(&self) -> Observed {
        *self.seen.borrow().last().expect("plan was never consulted")
    }
}

impl FetchPlan for ProbePlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        self.seen.borrow_mut().push(Observed {
            board_list: loaded.board_list(),
            column_list: loaded.column_list(),
            card_list: loaded.card_list(),
            sprint_list: loaded.sprint_list(),
            graph: loaded.graph(),
            column: loaded.column(self.column_id),
            card: loaded.card(self.card_id),
            sprint: loaded.sprint(self.sprint_id),
            columns_of_board: loaded.columns_of_board(self.board_id),
            cards_of_column: loaded.cards_of_column(self.column_id),
            sprints_of_board: loaded.sprints_of_board(self.board_id),
        });
        self.round.clone()
    }
}
