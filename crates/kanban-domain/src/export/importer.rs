//! Board import functionality.
//!
//! Supports both V1 (AllBoardsExport) and V2 (Snapshot with version envelope) formats.

use super::models::{AllBoardsExport, BoardExport};
use crate::{ArchivedCard, Board, Card, Column, Snapshot, Sprint};
use std::io;

/// Extracted entities from an import.
pub struct ImportedEntities {
    pub boards: Vec<Board>,
    pub columns: Vec<Column>,
    pub cards: Vec<Card>,
    pub archived_cards: Vec<ArchivedCard>,
    pub sprints: Vec<Sprint>,
}

/// Imports boards from JSON files.
pub struct BoardImporter;

impl BoardImporter {
    /// Try to load V2 format directly as a Snapshot.
    ///
    /// V2 format: `{"version": 2, "data": {...}}`
    /// Returns `Some(Snapshot)` if valid V2, `None` otherwise.
    pub fn try_load_snapshot(json: &str) -> Option<Snapshot> {
        let envelope: serde_json::Value = serde_json::from_str(json).ok()?;
        let version = envelope.get("version")?.as_u64()?;
        if version == 2 {
            let data = envelope.get("data")?;
            serde_json::from_value(data.clone()).ok()
        } else {
            None
        }
    }

    /// Import from JSON, supporting both V1 and V2 formats.
    ///
    /// - V1: `{"boards": [...]}`
    /// - V2: `{"version": 2, "data": {...}}`
    pub fn import_from_json(json: &str) -> Result<AllBoardsExport, io::Error> {
        // Try V2 format first
        if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(version) = envelope.get("version").and_then(|v| v.as_u64()) {
                if version == 2 {
                    // V2 format: data is a Snapshot with flat structure
                    if let Some(data) = envelope.get("data") {
                        if let Ok(snapshot) = serde_json::from_value::<Snapshot>(data.clone()) {
                            return Ok(Self::convert_snapshot_to_export(snapshot));
                        }
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "V2 format detected but data section is invalid".to_string(),
                    ));
                }
            }
        }

        // Fall back to V1 format (direct deserialization)
        serde_json::from_str(json).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid JSON format. Expected {{\"boards\": [...]}} structure (V1) or {{\"version\": 2, \"data\": {{...}}}} structure (V2). Error: {}",
                    err
                ),
            )
        })
    }

    /// Convert Snapshot format (V2) to AllBoardsExport format (V1-compatible).
    ///
    /// V2 has flat structure: boards[], columns[], cards[], sprints[]
    /// V1 has nested structure: boards[{board, columns[], cards[], sprints[]}]
    pub fn convert_snapshot_to_export(snapshot: Snapshot) -> AllBoardsExport {
        let mut board_exports = Vec::new();

        for board in snapshot.boards {
            let board_columns: Vec<_> = snapshot
                .columns
                .iter()
                .filter(|c| c.board_id == board.id)
                .cloned()
                .collect();

            let board_cards: Vec<_> = snapshot
                .cards
                .iter()
                .filter(|c| board_columns.iter().any(|col| col.id == c.column_id))
                .cloned()
                .collect();

            let board_sprints: Vec<_> = snapshot
                .sprints
                .iter()
                .filter(|s| s.board_id == board.id)
                .cloned()
                .collect();

            let board_archived: Vec<_> = snapshot
                .archived_cards
                .iter()
                .filter(|a| {
                    board_columns
                        .iter()
                        .any(|col| col.id == a.context.original_column_id)
                })
                .cloned()
                .collect();

            board_exports.push(BoardExport {
                board,
                columns: board_columns,
                cards: board_cards,
                sprints: board_sprints,
                archived_cards: board_archived,
            });
        }

        AllBoardsExport {
            boards: board_exports,
        }
    }

    /// Import from a file path.
    pub fn import_from_file(filename: &str) -> io::Result<AllBoardsExport> {
        let content = std::fs::read_to_string(filename)?;
        Self::import_from_json(&content)
    }

    /// Extract flat entity lists from an AllBoardsExport.
    pub fn extract_entities(import: AllBoardsExport) -> ImportedEntities {
        let mut boards = Vec::new();
        let mut columns = Vec::new();
        let mut cards = Vec::new();
        let mut archived_cards = Vec::new();
        let mut sprints = Vec::new();

        for board_data in import.boards {
            boards.push(board_data.board);
            columns.extend(board_data.columns);
            cards.extend(board_data.cards);
            archived_cards.extend(board_data.archived_cards);
            sprints.extend(board_data.sprints);
        }

        ImportedEntities {
            boards,
            columns,
            cards,
            archived_cards,
            sprints,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_from_json_v1_valid() {
        let json = r#"{
            "boards": [
                {
                    "board": {
                        "id": "550e8400-e29b-41d4-a716-446655440000",
                        "name": "Test Board",
                        "description": null,
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-01T00:00:00Z",
                        "sprint_prefix": null,
                        "card_prefix": null,
                        "task_sort_field": "Default",
                        "task_sort_order": "Ascending",
                        "active_sprint_id": null,
                        "sprint_duration_days": null,
                        "sprint_names": [],
                        "next_sprint_number": 1,
                        "sprint_name_used_count": 0,
                        "prefix_counters": {},
                        "sprint_counters": {},
                        "task_list_view": "Flat"
                    },
                    "columns": [],
                    "cards": [],
                    "archived_cards": [],
                    "sprints": []
                }
            ]
        }"#;

        let result = BoardImporter::import_from_json(json);
        assert!(result.is_ok());

        let import = result.unwrap();
        assert_eq!(import.boards.len(), 1);
        assert_eq!(import.boards[0].board.name, "Test Board");
    }

    #[test]
    fn test_import_from_json_invalid() {
        let json = r#"{ "invalid": "format" }"#;
        let result = BoardImporter::import_from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_entities() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let mut board_mut = board.clone();
        let card = Card::new(&mut board_mut, column.id, "Task", 0);

        let export = AllBoardsExport {
            boards: vec![BoardExport {
                board: board.clone(),
                columns: vec![column.clone()],
                cards: vec![card.clone()],
                archived_cards: vec![],
                sprints: vec![],
            }],
        };

        let entities = BoardImporter::extract_entities(export);

        assert_eq!(entities.boards.len(), 1);
        assert_eq!(entities.columns.len(), 1);
        assert_eq!(entities.cards.len(), 1);
        assert_eq!(entities.archived_cards.len(), 0);
        assert_eq!(entities.sprints.len(), 0);
    }

    #[test]
    fn test_try_load_snapshot_not_v2() {
        let json = r#"{"boards": []}"#;
        assert!(BoardImporter::try_load_snapshot(json).is_none());
    }

    #[test]
    fn test_convert_snapshot_to_export() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let snapshot = Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: crate::DependencyGraph::new(),
        };

        let export = BoardImporter::convert_snapshot_to_export(snapshot);
        assert_eq!(export.boards.len(), 1);
        assert_eq!(export.boards[0].board.name, "Test");
        assert_eq!(export.boards[0].columns.len(), 1);
    }
}
