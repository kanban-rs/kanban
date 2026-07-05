use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

use crate::{
    archival::ArchiveMetadata,
    board::BoardId,
    card::{Card, CardSummary},
    column::ColumnId,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedCard {
    #[serde(with = "crate::card_factory::card_serde")]
    pub card: Card,
    /// Shared archival envelope (`archived_at`, room to grow). `#[serde(flatten)]`
    /// keeps the on-disk shape byte-identical to the previous flat `archived_at`
    /// field, so no format bump / migration is needed.
    #[serde(flatten)]
    pub metadata: ArchiveMetadata,
    /// Board the card belonged to at archive time (D2 first-class model): a
    /// direct field so board-scoped queries need no column load. `#[serde(default)]`
    /// keeps pre-V8 files loadable (nil until the persistence migration backfills).
    #[serde(default)]
    pub board_id: BoardId,
    /// Historical column at archive time — NOT a live FK. May dangle if the
    /// column is later deleted; that is intentional under the first-class model.
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

impl ArchivedCard {
    pub fn new(
        card: Card,
        board_id: BoardId,
        original_column_id: ColumnId,
        original_position: i32,
    ) -> Self {
        Self {
            card,
            metadata: ArchiveMetadata::now(),
            board_id,
            original_column_id,
            original_position,
        }
    }

    pub fn into_card(self) -> Card {
        self.card
    }

    pub fn card_ref(&self) -> &Card {
        &self.card
    }

    pub fn card_mut(&mut self) -> &mut Card {
        &mut self.card
    }
}

impl From<ArchivedCard> for Card {
    fn from(archived_card: ArchivedCard) -> Self {
        archived_card.card
    }
}

impl Borrow<Card> for ArchivedCard {
    fn borrow(&self) -> &Card {
        &self.card
    }
}

impl crate::archival::ArchivedEntity for ArchivedCard {
    fn entity_id(&self) -> uuid::Uuid {
        self.card.id
    }

    fn metadata(&self) -> ArchiveMetadata {
        self.metadata
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedCardSummary {
    pub card: CardSummary,
    pub archived_at: DateTime<Utc>,
    pub board_id: BoardId,
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

impl From<&ArchivedCard> for ArchivedCardSummary {
    fn from(a: &ArchivedCard) -> Self {
        Self {
            card: CardSummary::from(&a.card),
            archived_at: a.metadata.archived_at,
            board_id: a.board_id,
            original_column_id: a.original_column_id,
            original_position: a.original_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archival::ArchivedEntity;
    use crate::{board::Board, card::Card, column::Column};

    fn sample() -> ArchivedCard {
        // Built via public constructors (the in-memory `test_support` module is
        // private and not usable from here).
        let mut board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0)
    }

    #[test]
    fn test_archived_card_implements_archived_entity() {
        let ac = sample();
        assert_eq!(ArchivedEntity::entity_id(&ac), ac.card.id);
        // The trait method returns the record's own metadata field.
        assert_eq!(ac.archived_at(), ac.metadata.archived_at);
    }

    #[test]
    fn test_metadata_flatten_keeps_archived_at_flat_on_the_wire() {
        // The `#[serde(flatten)]` metadata must serialize `archived_at` as a
        // TOP-LEVEL key (not nested under "metadata"), so the on-disk shape is
        // unchanged and no migration is needed.
        let ac = sample();
        let v = serde_json::to_value(&ac).unwrap();
        assert!(
            v.get("archived_at").is_some(),
            "archived_at stays top-level"
        );
        assert!(
            v.get("metadata").is_none(),
            "not nested under a metadata key"
        );
    }

    #[test]
    fn test_pre_metadata_flat_json_still_deserializes() {
        // A record written by the PREVIOUS (flat `archived_at`) code must still
        // load. Hand-assemble that historical shape - `archived_at` as a sibling
        // of `card`, NOT nested under a `metadata` key - so this genuinely fails
        // if `#[serde(flatten)]` is ever dropped (the real back-compat guard,
        // not a new->new round-trip that would move with the struct).
        let ac = sample();
        let card_value = serde_json::to_value(&ac)
            .unwrap()
            .get("card")
            .cloned()
            .expect("serialized card sub-object");
        let flat = serde_json::json!({
            "card": card_value,
            "archived_at": ac.metadata.archived_at,
            "original_column_id": ac.original_column_id,
            "original_position": ac.original_position,
        });
        let back: ArchivedCard = serde_json::from_value(flat).unwrap();
        assert_eq!(back, ac);
    }

    #[test]
    fn test_archived_card_retains_board_id() {
        // An archived card records the board it belonged to as its own field,
        // independent of any live column (D2 first-class model).
        let mut board = Board::new("B", None::<String>);
        let board_id = board.id;
        let col = Column::new(board_id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        let ac = ArchivedCard::new(card, board_id, col.id, 0);
        assert_eq!(ac.board_id, board_id);
    }

    #[test]
    fn test_archived_card_board_id_survives_json_round_trip() {
        // `#[serde(default)]` guarantees read-defaulting but NOT that the field
        // is written. Pin the write side too: a non-nil board_id must serialize
        // and reload intact, so it can never be paired with a silent skip.
        let mut board = Board::new("B", None::<String>);
        let board_id = board.id;
        let col = Column::new(board_id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        let ac = ArchivedCard::new(card, board_id, col.id, 0);
        let json = serde_json::to_string(&ac).unwrap();
        let restored: ArchivedCard = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.board_id, board_id);
    }

    #[test]
    fn test_archived_card_summary_carries_board_id() {
        // The summary projection must surface board_id so board-scoped queries
        // can filter without loading the full record.
        let ac = sample();
        let summary = ArchivedCardSummary::from(&ac);
        assert_eq!(summary.board_id, ac.board_id);
    }
}
