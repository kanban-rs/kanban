use serde::{Deserialize, Serialize};

use crate::{
    archival::{ArchiveMetadata, Archived},
    board::BoardId,
};

/// Archive context for a card: the board it belonged to, kept so board-scoped
/// archived queries need no column load (first-class `board_id`, KAN-829). The
/// card itself stays LIVE in `cards` under the reference-marker model, so there
/// is no original column/position to remember — nothing moves on archive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CardRestoreContext {
    #[serde(default)]
    pub board_id: BoardId,
}

/// An archived card: the shared [`Archived`] marker specialized with a card's
/// board context. A pure reference — `entity_id` points at the still-live card
/// in `cards`; the entity is never embedded here.
pub type ArchivedCard = Archived<CardRestoreContext>;

impl ArchivedCard {
    pub fn new(card_id: uuid::Uuid, board_id: BoardId) -> Self {
        Archived::with_context(
            card_id,
            CardRestoreContext { board_id },
            ArchiveMetadata::now(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archival::ArchivedEntity;

    #[test]
    fn test_archived_card_marker_exposes_entity_id_and_metadata() {
        let id = uuid::Uuid::new_v4();
        let ac = ArchivedCard::new(id, uuid::Uuid::nil());
        assert_eq!(ArchivedEntity::entity_id(&ac), id);
        assert_eq!(ac.entity_id, id);
        assert_eq!(ac.archived_at(), ac.metadata.archived_at);
    }

    #[test]
    fn test_archived_card_retains_board_id() {
        let board_id = uuid::Uuid::new_v4();
        let ac = ArchivedCard::new(uuid::Uuid::new_v4(), board_id);
        assert_eq!(ac.context.board_id, board_id);
    }

    #[test]
    fn test_metadata_and_context_flatten_flat_on_the_wire() {
        let ac = ArchivedCard::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let v = serde_json::to_value(ac).unwrap();
        assert!(
            v.get("archived_at").is_some(),
            "archived_at stays top-level"
        );
        assert!(v.get("board_id").is_some(), "board_id stays top-level");
        assert!(
            v.get("metadata").is_none(),
            "not nested under a metadata key"
        );
    }

    #[test]
    fn test_archived_card_marker_has_no_embedded_entity() {
        let ac = ArchivedCard::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let v = serde_json::to_value(ac).unwrap();
        assert!(
            v.get("entity").is_none() && v.get("card").is_none(),
            "the entity is referenced by id, never embedded"
        );
        assert_eq!(
            v.get("entity_id").and_then(|x| x.as_str()),
            Some(ac.entity_id.to_string().as_str())
        );
    }

    #[test]
    fn test_archived_card_marker_json_round_trip() {
        let ac = ArchivedCard::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let json = serde_json::to_string(&ac).unwrap();
        let restored: ArchivedCard = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ac);
    }
}
