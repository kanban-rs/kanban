//! Export data models.
//!
//! These DTOs represent the structure for import/export operations.

use crate::{ArchivedBoard, ArchivedCard, Board, Card, Column, Sprint};
use serde::{Deserialize, Serialize};

/// Export format for a single board with all its data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardExport {
    #[serde(with = "crate::board_factory::board_serde")]
    pub board: Board,
    #[serde(with = "crate::column_factory::column_vec_serde")]
    pub columns: Vec<Column>,
    #[serde(with = "crate::card_factory::card_vec_serde")]
    pub cards: Vec<Card>,
    #[serde(with = "crate::sprint_factory::sprint_vec_serde")]
    pub sprints: Vec<Sprint>,
    #[serde(default)]
    pub archived_cards: Vec<ArchivedCard>,
    #[serde(default)]
    pub archived_boards: Vec<ArchivedBoard>,
}

/// Export format for all boards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllBoardsExport {
    pub boards: Vec<BoardExport>,
}

impl AllBoardsExport {
    /// Create an empty export.
    pub fn empty() -> Self {
        Self { boards: Vec::new() }
    }

    /// Create from a list of board exports.
    pub fn from_boards(boards: Vec<BoardExport>) -> Self {
        Self { boards }
    }
}
