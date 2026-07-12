use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

use crate::{
    archival::{ArchivableEntity, ArchiveMetadata, Archived},
    board::BoardId,
    card::Card,
    column::ColumnId,
};

/// Restore context for an archived card. A card is a LEAF sharing a live
/// column, so it must remember where to go back. `board_id` is a direct field
/// (D2 first-class model) so board-scoped queries need no column load;
/// `#[serde(default)]` keeps pre-V8 files loadable (nil until backfilled).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardRestoreContext {
    #[serde(default)]
    pub board_id: BoardId,
    /// Historical column at archive time — NOT a live FK. May dangle if the
    /// column is later deleted; intentional under the first-class model.
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

/// An archived card: the shared [`Archived`] wrapper specialized to `Card` plus
/// its [`CardRestoreContext`]. On the wire: the card under `entity` (with a
/// `card` alias for already-shipped blobs), a flat `archived_at`, and the
/// flattened restore context.
pub type ArchivedCard = Archived<Card, CardRestoreContext>;

impl ArchivableEntity for Card {
    fn entity_id(&self) -> uuid::Uuid {
        self.id
    }

    fn serialize_entity<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        crate::card_factory::card_serde::serialize(self, s)
    }

    fn deserialize_entity<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        crate::card_factory::card_serde::deserialize(d)
    }
}

impl ArchivedCard {
    pub fn new(
        card: Card,
        board_id: BoardId,
        original_column_id: ColumnId,
        original_position: i32,
    ) -> Self {
        Archived::with_context(
            card,
            CardRestoreContext {
                board_id,
                original_column_id,
                original_position,
            },
            ArchiveMetadata::now(),
        )
    }

    pub fn into_card(self) -> Card {
        self.entity
    }

    pub fn card_ref(&self) -> &Card {
        &self.entity
    }

    pub fn card_mut(&mut self) -> &mut Card {
        &mut self.entity
    }
}

impl From<ArchivedCard> for Card {
    fn from(archived_card: ArchivedCard) -> Self {
        archived_card.entity
    }
}

impl Borrow<Card> for ArchivedCard {
    fn borrow(&self) -> &Card {
        &self.entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archival::ArchivedEntity;
    use crate::{board::Board, card::Card, column::Column};

    fn sample() -> ArchivedCard {
        let mut board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0)
    }

    #[test]
    fn test_archived_card_implements_archived_entity() {
        let ac = sample();
        assert_eq!(ArchivedEntity::entity_id(&ac), ac.entity.id);
        assert_eq!(ac.archived_at(), ac.metadata.archived_at);
    }

    #[test]
    fn test_metadata_flatten_keeps_archived_at_flat_on_the_wire() {
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
    fn test_legacy_card_keyed_json_still_deserializes() {
        // A record written by the PREVIOUS (bespoke `ArchivedCard`) code used a
        // `card` key for the entity. The `alias = "card"` on the generic must
        // keep it loadable — this is the real back-compat guard for the
        // migration to `Archived<Card, _>`.
        let ac = sample();
        let card_value = serde_json::to_value(&ac)
            .unwrap()
            .get("entity")
            .cloned()
            .expect("serialized entity sub-object");
        let legacy = serde_json::json!({
            "card": card_value,
            "archived_at": ac.metadata.archived_at,
            "board_id": ac.context.board_id,
            "original_column_id": ac.context.original_column_id,
            "original_position": ac.context.original_position,
        });
        let back: ArchivedCard = serde_json::from_value(legacy).unwrap();
        assert_eq!(back, ac);
    }

    #[test]
    fn test_archived_card_retains_board_id() {
        let mut board = Board::new("B", None::<String>);
        let board_id = board.id;
        let col = Column::new(board_id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        let ac = ArchivedCard::new(card, board_id, col.id, 0);
        assert_eq!(ac.context.board_id, board_id);
    }

    #[test]
    fn test_archived_card_board_id_survives_json_round_trip() {
        let mut board = Board::new("B", None::<String>);
        let board_id = board.id;
        let col = Column::new(board_id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        let ac = ArchivedCard::new(card, board_id, col.id, 0);
        let json = serde_json::to_string(&ac).unwrap();
        let restored: ArchivedCard = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.context.board_id, board_id);
    }
}
