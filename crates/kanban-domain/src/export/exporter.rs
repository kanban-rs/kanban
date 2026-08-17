//! Board export functionality.
//!
//! Converts domain entities into export format for serialization.

use super::models::{AllBoardsExport, BoardExport};
use crate::archival::ArchivedEntity;
use crate::{ArchivedBoard, ArchivedCard, Board, Card, Column, Sprint};
use std::collections::HashSet;
use std::io;
use uuid::Uuid;

/// Exports boards and their data to portable format.
pub struct BoardExporter;

impl BoardExporter {
    /// Export a single board with all its associated data.
    ///
    /// Archived cards are scoped by `board_id` (not column membership) so that
    /// cards whose original column was deleted after archival still round-trip.
    /// Their live card rows are included via a union of column-membership ids and
    /// archived-card entity ids, preventing orphaned markers on import.
    pub fn export_board(
        board: &Board,
        all_columns: &[Column],
        all_cards: &[Card],
        all_archived_cards: &[ArchivedCard],
        all_archived_boards: &[ArchivedBoard],
        all_sprints: &[Sprint],
    ) -> BoardExport {
        let board_columns: Vec<Column> = all_columns
            .iter()
            .filter(|col| col.board_id == board.id)
            .cloned()
            .collect();

        let column_ids: HashSet<Uuid> = board_columns.iter().map(|c| c.id).collect();

        // Scope archived cards by board_id (column may have been deleted post-archive).
        let board_archived_cards: Vec<ArchivedCard> = all_archived_cards
            .iter()
            .filter(|dc| dc.context.board_id == board.id)
            .cloned()
            .collect();

        // Carry live card rows for all archived cards in this board, even those
        // whose column_id dangles (column deleted after archival).
        let archived_card_ids: HashSet<Uuid> =
            board_archived_cards.iter().map(|a| a.entity_id()).collect();

        let board_cards: Vec<Card> = all_cards
            .iter()
            .filter(|card| {
                column_ids.contains(&card.column_id) || archived_card_ids.contains(&card.id)
            })
            .cloned()
            .collect();

        let board_archived_boards: Vec<ArchivedBoard> = all_archived_boards
            .iter()
            .filter(|ab| ab.entity_id() == board.id)
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
            archived_boards: board_archived_boards,
            sprints: board_sprints,
        }
    }

    /// Export all boards with their associated data.
    pub fn export_all_boards(
        boards: &[Board],
        columns: &[Column],
        cards: &[Card],
        archived_cards: &[ArchivedCard],
        archived_boards: &[ArchivedBoard],
        sprints: &[Sprint],
    ) -> AllBoardsExport {
        let board_exports: Vec<BoardExport> = boards
            .iter()
            .map(|board| {
                Self::export_board(
                    board,
                    columns,
                    cards,
                    archived_cards,
                    archived_boards,
                    sprints,
                )
            })
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
    use crate::archival::ArchivedEntity;

    #[test]
    fn test_export_single_board() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        let columns = vec![column.clone()];

        let board_mut = board.clone();
        let card = Card::new(board_mut.id, column.id, "Task", 0);
        let cards = vec![card];

        let archived_cards = vec![];
        let archived_boards = vec![];
        let sprints = vec![];

        let export = BoardExporter::export_board(
            &board,
            &columns,
            &cards,
            &archived_cards,
            &archived_boards,
            &sprints,
        );

        assert_eq!(export.board.name, "Test");
        assert_eq!(export.columns.len(), 1);
        assert_eq!(export.cards.len(), 1);
        assert_eq!(export.archived_cards.len(), 0);
        assert_eq!(export.archived_boards.len(), 0);
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
        let archived_boards = vec![];
        let sprints = vec![];

        let export = BoardExporter::export_all_boards(
            &boards,
            &columns,
            &cards,
            &archived_cards,
            &archived_boards,
            &sprints,
        );

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
                archived_boards: vec![],
                sprints: vec![],
            }],
        };

        let json = BoardExporter::export_to_json(&export).unwrap();
        assert!(json.contains("Test"));
    }

    #[test]
    fn test_export_board_carries_its_archived_board_marker() {
        let board = Board::new("Archived Board", None::<String>);
        let ab = ArchivedBoard::at(board.id, chrono::Utc::now());
        let columns = vec![];
        let cards = vec![];
        let archived_cards = vec![];
        let archived_boards = vec![ab];
        let sprints = vec![];

        let export = BoardExporter::export_board(
            &board,
            &columns,
            &cards,
            &archived_cards,
            &archived_boards,
            &sprints,
        );

        assert_eq!(export.archived_boards.len(), 1);
        assert_eq!(export.archived_boards[0].entity_id(), board.id);
    }

    #[test]
    fn test_export_board_omits_other_boards_archived_marker() {
        let board_a = Board::new("Board A", None::<String>);
        let board_b = Board::new("Board B", None::<String>);
        let ab_b = ArchivedBoard::at(board_b.id, chrono::Utc::now());
        let archived_boards = vec![ab_b];

        let export = BoardExporter::export_board(&board_a, &[], &[], &[], &archived_boards, &[]);

        assert_eq!(
            export.archived_boards.len(),
            0,
            "board A export must not carry board B's archived marker"
        );
    }

    #[test]
    fn test_export_board_carries_archived_card_row_with_dangling_column() {
        let board = Board::new("B", None::<String>);
        let live_col = Column::new(board.id, "Todo", 0);

        let board_mut = board.clone();
        let live_card = Card::new(board_mut.id, live_col.id, "Live", 0);

        // Archived card pointing at a DELETED column (dangling).
        let dangling_col_id = Uuid::new_v4();
        let archived_card_row = Card::new(board_mut.id, dangling_col_id, "Archived", 1);
        let ac_marker = crate::ArchivedCard::new(archived_card_row.id, board.id);

        let columns = vec![live_col.clone()];
        let cards = vec![live_card.clone(), archived_card_row.clone()];
        let archived_cards = vec![ac_marker];
        let archived_boards = vec![];
        let sprints = vec![];

        let export = BoardExporter::export_board(
            &board,
            &columns,
            &cards,
            &archived_cards,
            &archived_boards,
            &sprints,
        );

        assert_eq!(
            export.cards.len(),
            2,
            "live row of dangling-column archived card must be carried"
        );
        assert!(
            export.cards.iter().any(|c| c.id == archived_card_row.id),
            "dangling-column archived card live row must appear in export.cards"
        );
        assert_eq!(export.archived_cards.len(), 1);
        assert_eq!(export.archived_cards[0].entity_id(), archived_card_row.id);
    }
}
