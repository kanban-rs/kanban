use kanban_domain::{ArchivedCard, Board, Card, Column, DependencyGraph, Snapshot, Sprint};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Default)]
pub struct Model {
    boards: Option<Vec<Board>>,
    columns: Option<Vec<Column>>,
    cards: Option<Vec<Card>>,
    card_index: HashMap<Uuid, usize>,
    sprints: Option<Vec<Sprint>>,
    archived_cards: Option<Vec<ArchivedCard>>,
    archived_cards_flat: Option<Vec<Card>>,
    archived_card_index: HashMap<Uuid, usize>,
    graph: DependencyGraph,
}

impl Model {
    pub fn boards(&self) -> &[Board] {
        self.boards.as_deref().unwrap_or(&[])
    }

    pub fn columns(&self) -> &[Column] {
        self.columns.as_deref().unwrap_or(&[])
    }

    pub fn cards(&self) -> &[Card] {
        self.cards.as_deref().unwrap_or(&[])
    }

    pub fn card(&self, id: Uuid) -> Option<&Card> {
        let &idx = self.card_index.get(&id)?;
        self.cards.as_ref()?.get(idx)
    }

    pub fn sprints(&self) -> &[Sprint] {
        self.sprints.as_deref().unwrap_or(&[])
    }

    pub fn archived_cards(&self) -> &[ArchivedCard] {
        self.archived_cards.as_deref().unwrap_or(&[])
    }

    pub fn archived_cards_flat(&self) -> &[Card] {
        self.archived_cards_flat.as_deref().unwrap_or(&[])
    }

    pub fn archived_card(&self, id: Uuid) -> Option<&Card> {
        let &idx = self.archived_card_index.get(&id)?;
        self.archived_cards_flat.as_ref()?.get(idx)
    }

    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    pub fn load_from_snapshot(&mut self, snapshot: Snapshot) {
        self.card_index.clear();
        for (i, card) in snapshot.cards.iter().enumerate() {
            self.card_index.insert(card.id, i);
        }
        self.boards = Some(snapshot.boards);
        self.columns = Some(snapshot.columns);
        self.cards = Some(snapshot.cards);
        self.sprints = Some(snapshot.sprints);
        self.archived_card_index.clear();
        let mut flat = Vec::with_capacity(snapshot.archived_cards.len());
        for (i, ac) in snapshot.archived_cards.iter().enumerate() {
            self.archived_card_index.insert(ac.card.id, i);
            flat.push(ac.card.clone());
        }
        self.archived_cards = Some(snapshot.archived_cards);
        self.archived_cards_flat = Some(flat);
        self.graph = snapshot.graph;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{ArchivedCard, Board, Card, Column, Snapshot};

    fn make_card(board: &mut Board, column_id: Uuid) -> Card {
        Card::new(board, column_id, "task", 0)
    }

    #[test]
    fn test_default_model_returns_empty_slices() {
        let m = Model::default();
        assert!(m.boards().is_empty());
        assert!(m.columns().is_empty());
        assert!(m.cards().is_empty());
        assert!(m.sprints().is_empty());
        assert!(m.archived_cards().is_empty());
        assert!(m.archived_cards_flat().is_empty());
    }

    #[test]
    fn test_load_from_snapshot_populates_boards_and_columns() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        m.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![col.clone()],
            ..Default::default()
        });
        assert_eq!(m.boards().len(), 1);
        assert_eq!(m.boards()[0].id, board.id);
        assert_eq!(m.columns().len(), 1);
        assert_eq!(m.columns()[0].id, col.id);
    }

    #[test]
    fn test_card_lookup_by_id_returns_correct_card() {
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card_a = make_card(&mut board, col_id);
        let card_b = make_card(&mut board, col_id);
        let card_b_id = card_b.id;
        m.load_from_snapshot(Snapshot {
            cards: vec![card_a, card_b],
            ..Default::default()
        });
        let found = m.card(card_b_id).unwrap();
        assert_eq!(found.id, card_b_id);
    }

    #[test]
    fn test_card_lookup_missing_id_returns_none() {
        let m = Model::default();
        assert!(m.card(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_archived_card_lookup_by_id() {
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&mut board, col_id);
        let card_id = card.id;
        let archived = ArchivedCard::new(card, uuid::Uuid::nil(), col_id, 0);
        m.load_from_snapshot(Snapshot {
            archived_cards: vec![archived],
            ..Default::default()
        });
        let found = m.archived_card(card_id).unwrap();
        assert_eq!(found.id, card_id);
    }

    #[test]
    fn test_archived_card_lookup_missing_id_returns_none() {
        let m = Model::default();
        assert!(m.archived_card(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_archived_cards_flat_matches_archived_cards() {
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&mut board, col_id);
        let card_id = card.id;
        m.load_from_snapshot(Snapshot {
            archived_cards: vec![ArchivedCard::new(card, uuid::Uuid::nil(), col_id, 0)],
            ..Default::default()
        });
        assert_eq!(m.archived_cards_flat().len(), 1);
        assert_eq!(m.archived_cards_flat()[0].id, card_id);
    }

    #[test]
    fn test_load_from_snapshot_overwrites_previous_state() {
        let mut m = Model::default();
        let board_a = Board::new("A", None::<String>);
        m.load_from_snapshot(Snapshot {
            boards: vec![board_a],
            ..Default::default()
        });
        assert_eq!(m.boards().len(), 1);

        let board_b = Board::new("B", None::<String>);
        let board_c = Board::new("C", None::<String>);
        m.load_from_snapshot(Snapshot {
            boards: vec![board_b, board_c],
            ..Default::default()
        });
        assert_eq!(m.boards().len(), 2);
        assert_eq!(m.boards()[0].name, "B");
    }

    #[test]
    fn test_load_from_snapshot_clears_stale_card_index() {
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&mut board, col_id);
        let old_id = card.id;
        m.load_from_snapshot(Snapshot {
            cards: vec![card],
            ..Default::default()
        });
        assert!(m.card(old_id).is_some());

        // Reload with no cards — stale index entry must be gone
        m.load_from_snapshot(Snapshot::default());
        assert!(m.card(old_id).is_none());
    }
}
