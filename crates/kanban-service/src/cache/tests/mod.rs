use kanban_domain::{
    requestable, Board, Card, Column, DataStore, FetchPlan, FetchRound, FetchStatus, LoadedState,
    Sprint,
};
use std::cell::RefCell;
use uuid::Uuid;

use crate::read_recorder::RecordingStore;

mod invalidate;
mod loaded_view;
mod resolve;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Observed {
    board_list: FetchStatus,
    column_list: FetchStatus,
    card_list: FetchStatus,
    sprint_list: FetchStatus,
    graph: FetchStatus,
    column: FetchStatus,
    card: FetchStatus,
    sprint: FetchStatus,
}

struct ProbePlan {
    round: FetchRound,
    column_id: Uuid,
    card_id: Uuid,
    sprint_id: Uuid,
    seen: RefCell<Vec<Observed>>,
}

impl ProbePlan {
    fn new(round: FetchRound, column_id: Uuid, card_id: Uuid, sprint_id: Uuid) -> Self {
        Self {
            round,
            column_id,
            card_id,
            sprint_id,
            seen: RefCell::new(Vec::new()),
        }
    }

    fn last(&self) -> Observed {
        *self.seen.borrow().last().expect("plan was never consulted")
    }
}

impl FetchPlan for ProbePlan {
    fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound {
        self.seen.borrow_mut().push(Observed {
            board_list: loaded.board_list(),
            column_list: loaded.column_list(),
            card_list: loaded.card_list(),
            sprint_list: loaded.sprint_list(),
            graph: loaded.graph(),
            column: loaded.column(self.column_id),
            card: loaded.card(self.card_id),
            sprint: loaded.sprint(self.sprint_id),
        });
        self.round.clone()
    }
}

fn store() -> RecordingStore {
    RecordingStore::new()
}

fn seed_board(store: &RecordingStore, name: &str) -> Board {
    let board = Board::new(name, None::<String>);
    store.upsert_board(board.clone()).unwrap();
    board
}

fn seed_column(store: &RecordingStore, board: &Board, name: &str) -> Column {
    let column = Column::new(board.id, name, 0);
    store.upsert_column(column.clone()).unwrap();
    column
}

fn seed_card(store: &RecordingStore, board: &Board, column: &Column, title: &str) -> Card {
    let card = Card::new(board.id, column.id, title, 0);
    store.upsert_card(card.clone()).unwrap();
    card
}

fn seed_sprint(store: &RecordingStore, board: &Board) -> Sprint {
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    store.upsert_sprint(sprint.clone()).unwrap();
    sprint
}

fn seed_board_with_column(store: &RecordingStore) -> (Board, Column) {
    let board = seed_board(store, "b");
    let column = seed_column(store, &board, "c");
    (board, column)
}

struct FixedPlan(FetchRound);

impl FetchPlan for FixedPlan {
    fn next_round(&self, _loaded: &dyn LoadedState) -> FetchRound {
        self.0.clone()
    }
}

struct BoardListPlan;

impl FetchPlan for BoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound {
        FetchRound {
            board_list: requestable(loaded.board_list()),
            ..Default::default()
        }
    }
}

struct GraphThenCardPlan {
    card_id: Uuid,
}

impl FetchPlan for GraphThenCardPlan {
    fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound {
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
