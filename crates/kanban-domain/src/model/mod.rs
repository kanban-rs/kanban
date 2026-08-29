use crate::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, LoadState, Snapshot, Sprint,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct Model {
    boards: LoadState<Vec<Board>>,
    columns: LoadState<Vec<Column>>,
    cards: LoadState<Vec<Card>>,
    card_index: HashMap<Uuid, usize>,
    board_index: HashMap<Uuid, usize>,
    sprints: LoadState<Vec<Sprint>>,
    archived_cards: Option<Vec<ArchivedCard>>,
    archived_card_ids: HashSet<Uuid>,
    archived_boards: Option<Vec<ArchivedBoard>>,
    archived_board_ids: HashSet<Uuid>,
    graph: LoadState<DependencyGraph>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            boards: LoadState::NotLoaded,
            columns: LoadState::NotLoaded,
            cards: LoadState::NotLoaded,
            card_index: HashMap::new(),
            board_index: HashMap::new(),
            sprints: LoadState::NotLoaded,
            archived_cards: None,
            archived_card_ids: HashSet::new(),
            archived_boards: None,
            archived_board_ids: HashSet::new(),
            graph: LoadState::NotLoaded,
        }
    }
}

impl Model {
    pub fn load_from_snapshot(&mut self, _snapshot: Snapshot) {
        todo!()
    }

    fn rebuild_card_index(&mut self) {
        todo!()
    }

    fn rebuild_board_index(&mut self) {
        todo!()
    }
}

mod boards;
mod cards;
mod collections;
mod graph;

#[cfg(any(test, feature = "test-helpers"))]
mod test_helpers;
#[cfg(any(test, feature = "test-helpers"))]
pub use test_helpers::ModelLoadStates;
