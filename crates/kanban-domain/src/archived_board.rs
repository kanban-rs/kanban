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

