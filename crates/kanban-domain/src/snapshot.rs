//! Point-in-time capture of all kanban data.
//!
//! The `Snapshot` type provides a serializable representation of all domain
//! state. It is used for:
//! - Persistence (saving/loading to disk)
//! - Import/export functionality
//! - Undo/redo history (capturing state before mutations)
//!
//! This type is pure data with no UI dependencies, making it suitable for
//! use by both TUI and future API server implementations.

use crate::{ArchivedCard, Board, Card, Column, DependencyGraph, Sprint};
use serde::{Deserialize, Serialize};

/// Point-in-time capture of all kanban data.
///
/// Contains the complete state of boards, columns, cards, sprints,
/// archived cards, and the dependency graph. All fields use `#[serde(default)]`
/// to support partial snapshots and backward compatibility with older formats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Snapshot {
    /// All boards in the workspace.
    #[serde(default, with = "crate::board_factory::board_vec_serde")]
    pub boards: Vec<Board>,

    /// All columns across all boards.
    #[serde(default)]
    pub columns: Vec<Column>,

    /// All active cards.
    #[serde(default)]
    pub cards: Vec<Card>,

    /// All archived cards.
    #[serde(default)]
    pub archived_cards: Vec<ArchivedCard>,

    /// All sprints across all boards.
    #[serde(default)]
    pub sprints: Vec<Sprint>,

    /// Card dependency graph (blocks, relates-to, parent-child).
    #[serde(default)]
    pub graph: DependencyGraph,
}

impl Snapshot {
    /// Create an empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a snapshot from component data.
    pub fn from_data(
        boards: Vec<Board>,
        columns: Vec<Column>,
        cards: Vec<Card>,
        archived_cards: Vec<ArchivedCard>,
        sprints: Vec<Sprint>,
        graph: DependencyGraph,
    ) -> Self {
        Self {
            boards,
            columns,
            cards,
            archived_cards,
            sprints,
            graph,
        }
    }

    /// Check if the snapshot is empty (no data).
    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
            && self.columns.is_empty()
            && self.cards.is_empty()
            && self.archived_cards.is_empty()
            && self.sprints.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_snapshot() {
        let snapshot = Snapshot::new();
        assert!(snapshot.is_empty());
        assert!(snapshot.boards.is_empty());
        assert!(snapshot.columns.is_empty());
        assert!(snapshot.cards.is_empty());
    }

    #[test]
    fn test_snapshot_from_data() {
        let board = Board::new("Test", None::<String>);
        let snapshot = Snapshot::from_data(
            vec![board.clone()],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        assert!(!snapshot.is_empty());
        assert_eq!(snapshot.boards.len(), 1);
        assert_eq!(snapshot.boards[0].name, "Test");
    }

    #[test]
    fn test_snapshot_serialization_roundtrip() {
        let board = Board::new("Test Board", None::<String>);
        let snapshot = Snapshot::from_data(
            vec![board],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        // Verify key data survived the roundtrip
        assert_eq!(restored.boards.len(), 1);
        assert_eq!(restored.boards[0].name, "Test Board");
        assert!(restored.columns.is_empty());
    }

    #[test]
    fn test_snapshot_partial_eq() {
        let snap1 = Snapshot::new();
        let snap2 = Snapshot::new();
        assert_eq!(snap1, snap2);
    }

    #[test]
    fn test_snapshot_partial_deserialization() {
        // Test that missing fields default correctly (backward compatibility)
        let json = r#"{"boards": []}"#;
        let snapshot: Snapshot = serde_json::from_str(json).unwrap();

        assert!(snapshot.columns.is_empty());
        assert!(snapshot.cards.is_empty());
        assert!(snapshot.sprints.is_empty());
    }

    fn fully_populated_board() -> Board {
        use crate::board_factory::BoardRecord;
        use crate::task_list_view::TaskListView;
        use crate::{SortField, SortOrder};
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut sprint_counters = HashMap::new();
        sprint_counters.insert("SPR".to_string(), 4);
        let record = BoardRecord {
            id: Uuid::new_v4(),
            name: "Populated".to_string(),
            description: Some("d".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: SortField::Priority,
            task_sort_order: SortOrder::Descending,
            sprint_duration_days: Some(14),
            sprint_names: vec!["Alpha".to_string(), "Beta".to_string()],
            sprint_name_used_count: 1,
            next_sprint_number: 12,
            active_sprint_id: Some(Uuid::new_v4()),
            task_list_view: TaskListView::GroupedByColumn,
            card_counter: 99,
            sprint_counters,
            completion_column_id: Some(Uuid::new_v4()),
            position: 5,
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
        };
        Board::reconstitute(record).unwrap()
    }

    #[test]
    fn test_json_board_round_trip_through_record_is_identity() {
        let board = fully_populated_board();
        let snapshot = Snapshot::from_data(
            vec![board.clone()],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.boards.len(), 1);
        assert_eq!(restored.boards[0], board);
    }

    #[test]
    fn test_json_board_legacy_v_migration_still_round_trips() {
        // A V2-shaped board: prefix_counters set, no card_counter. The boards
        // field routes through BoardRecord's migration Deserialize.
        let json = r#"{
            "boards": [{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "name": "Legacy",
                "description": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "sprint_prefix": null,
                "card_prefix": "feat",
                "task_sort_field": "Default",
                "task_sort_order": "Ascending",
                "active_sprint_id": null,
                "sprint_duration_days": null,
                "sprint_names": [],
                "next_sprint_number": 1,
                "sprint_name_used_count": 0,
                "prefix_counters": {"feat": 42, "other": 5},
                "sprint_counters": {},
                "task_list_view": "Flat"
            }]
        }"#;
        let snapshot: Snapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.boards.len(), 1);
        assert_eq!(snapshot.boards[0].card_counter, 42);
    }

    #[test]
    fn test_json_board_load_rejects_malformed_record() {
        // A board object missing the required `id` field must error on load,
        // not silently default.
        let json = r#"{
            "boards": [{
                "name": "No Id",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let result: Result<Snapshot, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
