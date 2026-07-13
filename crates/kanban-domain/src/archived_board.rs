use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    archival::{ArchivableEntity, Archived},
    board::{Board, BoardId},
};

impl ArchivableEntity for Board {
    fn entity_id(&self) -> Uuid {
        self.id
    }

    fn serialize_entity<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        crate::board_factory::board_serde::serialize(self, s)
    }

    fn deserialize_entity<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        crate::board_factory::board_serde::deserialize(d)
    }
}

/// An archived board: the shared [`Archived`] wrapper specialized to `Board`. A
/// board is a scoping ROOT — its subtree (columns / cards / archived_cards /
/// sprints / edges) stays in place in the flat collections — so it needs no
/// restore context (`NoContext`). Archiving moves the board head into the
/// wrapper; restore (`into_entity`) unwraps it, losslessly. On the wire: the
/// board under `entity`, a flat `archived_at`, no context.
pub type ArchivedBoard = Archived<Board>;

/// Lightweight projection for the archived-boards list view. Reads the board
/// head plus archive time only — no subtree counts (never gathered under the
/// discrete-collection model). `archived_at` is non-optional (it always exists
/// on an archived record).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedBoardSummary {
    pub board_id: BoardId,
    pub name: String,
    pub description: Option<String>,
    pub archived_at: DateTime<Utc>,
    pub position: i32,
}

impl From<&ArchivedBoard> for ArchivedBoardSummary {
    fn from(ab: &ArchivedBoard) -> Self {
        Self {
            board_id: ab.entity.id,
            name: ab.entity.name.clone(),
            description: ab.entity.description.clone(),
            archived_at: ab.metadata.archived_at,
            position: ab.entity.position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archival::ArchivedEntity;

    #[test]
    fn test_archived_board_wraps_and_restores_losslessly() {
        let board = Board::new("Proj", Some("KAN"));
        let ab = ArchivedBoard::now(board.clone());
        assert_eq!(ab.entity_id(), board.id);
        assert_eq!(
            ab.into_entity(),
            board,
            "restore returns the board verbatim"
        );
    }

    #[test]
    fn test_archived_board_round_trips_json_entity_keyed_flat_archived_at() {
        let ts = Utc::now();
        let ab = ArchivedBoard::at(Board::new("B", Some("KAN")), ts);
        let v = serde_json::to_value(&ab).unwrap();
        assert!(v.get("entity").is_some(), "board is under the entity key");
        assert!(v.get("archived_at").is_some(), "archived_at is flat");
        let back: ArchivedBoard = serde_json::from_value(v).unwrap();
        assert_eq!(back, ab);
        assert_eq!(back.archived_at(), ts);
    }

    #[test]
    fn test_archived_board_summary_projects_head_and_archived_at() {
        let ts = Utc::now();
        let mut board = Board::new("Proj", None::<String>);
        board.description = Some("d".to_string());
        board.position = 4;
        let ab = ArchivedBoard::at(board.clone(), ts);
        let s = ArchivedBoardSummary::from(&ab);
        assert_eq!(s.board_id, board.id);
        assert_eq!(s.name, "Proj");
        assert_eq!(s.description, Some("d".to_string()));
        assert_eq!(s.position, 4);
        assert_eq!(s.archived_at, ts);
    }
}
