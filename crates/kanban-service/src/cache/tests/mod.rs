use kanban_domain::{
    requestable, Board, Card, Column, DataStore, FetchPlan, FetchRound, LoadedState, Sprint,
};
use uuid::Uuid;

use crate::read_recorder::RecordingStore;

mod invalidate;
mod resolve;

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
