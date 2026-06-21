use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{ArchivedCard, Board, Card, Column, DependencyGraph, Sprint};

#[derive(Debug, Clone)]
pub(super) struct StoreState {
    pub(super) boards: HashMap<Uuid, Board>,
    pub(super) columns: HashMap<Uuid, Column>,
    pub(super) cards: HashMap<Uuid, Card>,
    /// Secondary index: column_id -> set of card ids currently in that column.
    /// Maintained transactionally by every method that mutates `cards`'s
    /// column membership (upsert_card, delete_card, delete_cards_by_columns,
    /// apply_snapshot). Lets `count_cards_in_column_excluding` and friends
    /// run in O(column_size) instead of O(total_cards).
    pub(super) cards_by_column: HashMap<Uuid, HashSet<Uuid>>,
    pub(super) sprints: HashMap<Uuid, Sprint>,
    pub(super) archived_cards: HashMap<Uuid, ArchivedCard>,
    pub(super) graph: DependencyGraph,
}

impl StoreState {
    pub(super) fn new() -> Self {
        Self {
            boards: HashMap::new(),
            columns: HashMap::new(),
            cards: HashMap::new(),
            cards_by_column: HashMap::new(),
            sprints: HashMap::new(),
            archived_cards: HashMap::new(),
            graph: DependencyGraph::new(),
        }
    }

    pub(super) fn add_card_to_column_index(&mut self, card_id: Uuid, column_id: Uuid) {
        self.cards_by_column
            .entry(column_id)
            .or_default()
            .insert(card_id);
    }

    pub(super) fn remove_card_from_column_index(&mut self, card_id: Uuid, column_id: Uuid) {
        if let Some(set) = self.cards_by_column.get_mut(&column_id) {
            set.remove(&card_id);
            if set.is_empty() {
                self.cards_by_column.remove(&column_id);
            }
        }
    }

    pub(super) fn rebuild_card_column_index(&mut self) {
        self.cards_by_column.clear();
        for card in self.cards.values() {
            self.cards_by_column
                .entry(card.column_id)
                .or_default()
                .insert(card.id);
        }
    }
}
