use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    archival::{Archived, NoContext},
    board::{Board, BoardId},
};

/// An archived board: the shared [`Archived`] marker for a scoping ROOT. A board
/// needs no restore context (`NoContext`) — its subtree (columns / cards /
/// sprints / edges) stays in place in the flat collections, and the board head
/// itself stays LIVE in `boards` under the reference-marker model. `entity_id`
/// points at that still-live board; the board is never embedded here.
pub type ArchivedBoard = Archived<NoContext>;

/// Lightweight projection for the archived-boards list view. Reads the live
/// board head plus archive time only — no subtree counts (never gathered under
/// the discrete-collection model). `archived_at` is non-optional (it always
/// exists on an archived record).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedBoardSummary {
    pub board_id: BoardId,
    pub name: String,
    pub description: Option<String>,
    pub archived_at: DateTime<Utc>,
    pub position: i32,
}

impl ArchivedBoardSummary {
    /// Project the live board head plus the marker's archive time. The board is
    /// no longer embedded in the marker, so the caller supplies the live `Board`
    /// (fetched by `ab.entity_id`) explicitly.
    pub fn from_marker(board: &Board, ab: &ArchivedBoard) -> Self {
        Self {
            board_id: board.id,
            name: board.name.clone(),
            description: board.description.clone(),
            archived_at: ab.metadata.archived_at,
            position: board.position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archival::ArchivedEntity;

    #[test]
    fn test_archived_board_marker_exposes_entity_id_and_archived_at() {
        let ts = Utc::now();
        let id = uuid::Uuid::new_v4();
        let ab = ArchivedBoard::at(id, ts);
        assert_eq!(ab.entity_id(), id);
        assert_eq!(ab.archived_at(), ts);
    }

    #[test]
    fn test_archived_board_marker_round_trips_json_flat_archived_at() {
        let ts = Utc::now();
        let ab = ArchivedBoard::at(uuid::Uuid::new_v4(), ts);
        let v = serde_json::to_value(ab).unwrap();
        assert!(
            v.get("entity").is_none(),
            "board is referenced, not embedded"
        );
        assert!(v.get("archived_at").is_some(), "archived_at is flat");
        let back: ArchivedBoard = serde_json::from_value(v).unwrap();
        assert_eq!(back, ab);
        assert_eq!(back.archived_at(), ts);
    }

    #[test]
    fn test_archived_board_summary_projects_live_head_and_archived_at() {
        let ts = Utc::now();
        let mut board = Board::new("Proj", None::<String>);
        board.description = Some("d".to_string());
        board.position = 4;
        let ab = ArchivedBoard::at(board.id, ts);
        let s = ArchivedBoardSummary::from_marker(&board, &ab);
        assert_eq!(s.board_id, board.id);
        assert_eq!(s.name, "Proj");
        assert_eq!(s.description, Some("d".to_string()));
        assert_eq!(s.position, 4);
        assert_eq!(s.archived_at, ts);
    }
}
