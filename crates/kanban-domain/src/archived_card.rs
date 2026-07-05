use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

use crate::{
    archival::ArchiveMetadata,
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
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

impl ArchivedCard {
    pub fn new(card: Card, original_column_id: ColumnId, original_position: i32) -> Self {
        Self {
            card,
            metadata: ArchiveMetadata::now(),
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
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

impl From<&ArchivedCard> for ArchivedCardSummary {
    fn from(a: &ArchivedCard) -> Self {
        Self {
            card: CardSummary::from(&a.card),
            archived_at: a.metadata.archived_at,
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
        ArchivedCard::new(card, col.id, 0)
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
        // A record written by the previous (flat `archived_at`) code round-trips
        // into the new shape unchanged — the back-compat proof that this is not
        // a breaking format change.
        let ac = sample();
        let json = serde_json::to_string(&ac).unwrap();
        let back: ArchivedCard = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ac);
        assert_eq!(back.metadata.archived_at, ac.metadata.archived_at);
    }
}
