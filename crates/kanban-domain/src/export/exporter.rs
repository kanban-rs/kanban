//! Board export functionality.
//!
//! Converts domain entities into export format for serialization.

use super::models::{AllBoardsExport, BoardExport};
use crate::{ArchivedCard, Board, Card, Column, Sprint};
use std::io;
use uuid::Uuid;

/// Exports boards and their data to portable format.
pub struct BoardExporter;

impl BoardExporter {
    /// Export a single board with all its associated data.
    pub fn export_board(
        board: &Board,
        all_columns: &[Column],
        all_cards: &[Card],
        all_archived_cards: &[ArchivedCard],
        all_sprints: &[Sprint],
    ) -> BoardExport {
        let board_columns: Vec<Column> = all_columns
            .iter()
            .filter(|col| col.board_id == board.id)
            .cloned()
            .collect();

        let column_ids: Vec<Uuid> = board_columns.iter().map(|c| c.id).collect();

        let board_cards: Vec<Card> = all_cards
            .iter()
            .filter(|card| column_ids.contains(&card.column_id))
            .cloned()
            .collect();

        let board_archived_cards: Vec<ArchivedCard> = all_archived_cards
            .iter()
            .filter(|dc| column_ids.contains(&dc.context.original_column_id))
            .cloned()
            .collect();

        let board_sprints: Vec<Sprint> = all_sprints
            .iter()
            .filter(|s| s.board_id == board.id)
            .cloned()
            .collect();

        BoardExport {
            board: board.clone(),
            columns: board_columns,
            cards: board_cards,
            archived_cards: board_archived_cards,
            sprints: board_sprints,
        }
    }

    /// Export all boards with their associated data.
    pub fn export_all_boards(
        boards: &[Board],
        columns: &[Column],
        cards: &[Card],
        archived_cards: &[ArchivedCard],
        sprints: &[Sprint],
    ) -> AllBoardsExport {
        let board_exports: Vec<BoardExport> = boards
            .iter()
            .map(|board| Self::export_board(board, columns, cards, archived_cards, sprints))
            .collect();

        AllBoardsExport {
            boards: board_exports,
        }
    }

    /// Serialize export to JSON string.
    pub fn export_to_json(export: &AllBoardsExport) -> Result<String, io::Error> {
        serde_json::to_string_pretty(export).map_err(io::Error::other)
    }

    /// Export directly to a file.
    pub fn export_to_file(export: &AllBoardsExport, filename: &str) -> io::Result<()> {
        let json = Self::export_to_json(export)?;
        std::fs::write(filename, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_single_board() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        let columns = vec![column.clone()];

        let mut board_mut = board.clone();
        let card = Card::new(&mut board_mut, column.id, "Task", 0);
        let cards = vec![card];

        let archived_cards = vec![];
        let sprints = vec![];

        let export =
            BoardExporter::export_board(&board, &columns, &cards, &archived_cards, &sprints);

        assert_eq!(export.board.name, "Test");
        assert_eq!(export.columns.len(), 1);
        assert_eq!(export.cards.len(), 1);
        assert_eq!(export.archived_cards.len(), 0);
    }

    #[test]
    fn test_export_all_boards() {
        let board1 = Board::new("Board 1", None::<String>);
        let board2 = Board::new("Board 2", None::<String>);
        let boards = vec![board1.clone(), board2.clone()];

        let column1 = Column::new(board1.id, "Todo", 0);
        let column2 = Column::new(board2.id, "Todo", 0);
        let columns = vec![column1.clone(), column2.clone()];

        let cards = vec![];
        let archived_cards = vec![];
        let sprints = vec![];

        let export =
            BoardExporter::export_all_boards(&boards, &columns, &cards, &archived_cards, &sprints);

        assert_eq!(export.boards.len(), 2);
        assert_eq!(export.boards[0].board.name, "Board 1");
        assert_eq!(export.boards[1].board.name, "Board 2");
    }

    #[test]
    fn test_export_to_json() {
        let board = Board::new("Test", None::<String>);
        let export = AllBoardsExport {
            boards: vec![BoardExport {
                board,
                columns: vec![],
                cards: vec![],
                archived_cards: vec![],
                sprints: vec![],
            }],
        };

        let json = BoardExporter::export_to_json(&export).unwrap();
        assert!(json.contains("Test"));
    }
}
