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

use crate::{ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, Sprint};
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
    #[serde(default, with = "crate::column_factory::column_vec_serde")]
    pub columns: Vec<Column>,

    /// All active cards.
    #[serde(default, with = "crate::card_factory::card_vec_serde")]
    pub cards: Vec<Card>,

    /// All archived cards.
    #[serde(default)]
    pub archived_cards: Vec<ArchivedCard>,

    /// All sprints across all boards.
    #[serde(default, with = "crate::sprint_factory::sprint_vec_serde")]
    pub sprints: Vec<Sprint>,

    /// Archived boards — the discrete, first-class peer collection to `boards`,
    /// holding `Archived<Board>` wrappers just as `archived_cards` holds
    /// `Archived<Card, _>`. Each board's subtree (columns/cards/archived_cards/
    /// sprints/edges) stays in place in the flat collections above; only the
    /// board head moves into its wrapper.
    #[serde(default)]
    pub archived_boards: Vec<ArchivedBoard>,

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
            archived_boards: Vec::new(),
        }
    }

    /// Check if the snapshot is empty (no data).
    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
            && self.columns.is_empty()
            && self.cards.is_empty()
            && self.archived_cards.is_empty()
            && self.sprints.is_empty()
            && self.archived_boards.is_empty()
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

    fn populated_columns() -> Vec<Column> {
        use crate::column_factory::ColumnRecord;
        use uuid::Uuid;

        let board_id = Uuid::new_v4();
        let make = |position: i32, wip_limit: Option<i32>| {
            Column::reconstitute(ColumnRecord {
                id: Uuid::new_v4(),
                board_id,
                name: format!("Col {position}"),
                position,
                wip_limit,
                default_status: None,
                created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
                updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
            })
            .unwrap()
        };
        vec![make(0, Some(7)), make(1, None), make(2, Some(0))]
    }

    #[test]
    fn test_json_snapshot_column_round_trip() {
        let columns = populated_columns();
        let snapshot = Snapshot::from_data(
            vec![],
            columns.clone(),
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.columns, columns);
    }

    #[test]
    fn test_json_snapshot_column_deserialize_uses_record() {
        // A column object missing the required `id` field must error on load at
        // the ColumnRecord boundary, not silently default. This compile-locks
        // that ColumnRecord (not Column) owns the serde edge.
        let json = r#"{
            "columns": [{
                "board_id": "550e8400-e29b-41d4-a716-446655440000",
                "name": "No Id",
                "position": 0,
                "wip_limit": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let result: Result<Snapshot, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    fn fully_populated_card() -> Card {
        use crate::card_factory::CardRecord;
        use crate::{CardPriority, CardStatus, SprintLog};
        use uuid::Uuid;

        let sprint_id = Uuid::new_v4();
        let record = CardRecord {
            id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            title: "Done card".to_string(),
            description: Some("finished".to_string()),
            priority: CardPriority::High,
            status: CardStatus::Done,
            position: 7,
            due_date: Some("2024-05-05T00:00:00Z".parse().unwrap()),
            points: Some(3),
            card_number: 42,
            sprint_id: Some(sprint_id),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
            completed_at: Some("2024-03-03T00:00:00Z".parse().unwrap()),
            sprint_logs: vec![
                SprintLog {
                    sprint_id,
                    sprint_number: 1,
                    sprint_name: Some("Sprint 1".to_string()),
                    started_at: "2024-01-10T00:00:00Z".parse().unwrap(),
                    ended_at: Some("2024-01-20T00:00:00Z".parse().unwrap()),
                    status: "Completed".to_string(),
                },
                SprintLog {
                    sprint_id,
                    sprint_number: 2,
                    sprint_name: None,
                    started_at: "2024-02-01T00:00:00Z".parse().unwrap(),
                    ended_at: None,
                    status: "Active".to_string(),
                },
            ],
        };
        Card::reconstitute(record).unwrap()
    }

    #[test]
    fn test_json_card_round_trip_preserves_all_fields() {
        let card = fully_populated_card();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![card.clone()],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.cards.len(), 1);
        assert_eq!(restored.cards[0], card);
    }

    #[test]
    fn test_json_card_round_trip_preserves_sprint_logs_verbatim() {
        let card = fully_populated_card();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![card.clone()],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.cards[0].sprint_logs, card.sprint_logs);
        assert_eq!(restored.cards[0].sprint_logs.len(), 2);
        assert_eq!(restored.cards[0].sprint_logs[1].ended_at, None);
    }

    #[test]
    fn test_json_archived_card_round_trip_preserves_card() {
        use uuid::Uuid;
        // Reference-marker model: the archived card stays LIVE in `cards`; the
        // `archived_cards` entry is a pure marker referencing it by `entity_id`.
        let card = fully_populated_card();
        let board_id = Uuid::new_v4();
        let archived = ArchivedCard::new(card.id, board_id);
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![card.clone()],
            vec![archived],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        // The live card survives verbatim in `cards`.
        assert_eq!(restored.cards.len(), 1);
        assert_eq!(restored.cards[0], card);
        // The marker survives and references the live card by id.
        assert_eq!(restored.archived_cards.len(), 1);
        assert_eq!(restored.archived_cards[0].entity_id, card.id);
        assert_eq!(restored.archived_cards[0].context.board_id, board_id);
        assert_eq!(restored.archived_cards[0], archived);
    }

    #[test]
    fn test_archived_card_board_id_defaults_when_absent_in_json() {
        // A pre-V8 snapshot has no `board_id` on its archived cards. The
        // `#[serde(default)]` must let it load with a nil board_id (the correct
        // backfill is the persistence migration's job, D7), so old files parse.
        use uuid::Uuid;
        let card = fully_populated_card();
        let archived = ArchivedCard::new(card.id, Uuid::new_v4());
        let mut value = serde_json::to_value(archived).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("board_id")
            .expect("serialized form carries board_id");
        let restored: ArchivedCard = serde_json::from_value(value).unwrap();
        assert_eq!(restored.context.board_id, Uuid::nil());
    }

    #[test]
    fn test_json_card_deserialize_uses_record() {
        // A card object missing the required `id` field must error on load at the
        // CardRecord boundary, not silently default. Compile-locks that CardRecord
        // (not Card) owns the serde edge.
        let json = r#"{
            "cards": [{
                "column_id": "550e8400-e29b-41d4-a716-446655440002",
                "title": "No Id",
                "description": null,
                "priority": "Medium",
                "status": "Todo",
                "position": 0,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let result: Result<Snapshot, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    fn fully_populated_sprint() -> Sprint {
        use crate::sprint_factory::SprintRecord;
        use crate::SprintStatus;
        use uuid::Uuid;

        let record = SprintRecord {
            id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            sprint_number: 9,
            name_index: Some(4),
            prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            status: SprintStatus::Completed,
            start_date: Some("2024-02-01T00:00:00Z".parse().unwrap()),
            end_date: Some("2024-02-14T00:00:00Z".parse().unwrap()),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-15T00:00:00Z".parse().unwrap(),
        };
        Sprint::reconstitute(record).unwrap()
    }

    #[test]
    fn test_json_sprint_round_trip_through_record_is_identity() {
        let sprint = fully_populated_sprint();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![sprint.clone()],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.sprints.len(), 1);
        assert_eq!(restored.sprints[0], sprint);
    }

    #[test]
    fn test_json_sprint_round_trips_completed_lifecycle_dates() {
        use crate::SprintStatus;
        let sprint = fully_populated_sprint();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![sprint.clone()],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.sprints[0].status, SprintStatus::Completed);
        assert_eq!(restored.sprints[0].start_date, sprint.start_date);
        assert_eq!(restored.sprints[0].end_date, sprint.end_date);
    }

    #[test]
    fn test_json_snapshot_loads_legacy_prefix_override_alias_for_sprint() {
        let json = r#"{
            "sprints": [{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "board_id": "550e8400-e29b-41d4-a716-446655440001",
                "sprint_number": 1,
                "name_index": null,
                "prefix_override": "OLD",
                "card_prefix": null,
                "status": "Planning",
                "start_date": null,
                "end_date": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let snapshot: Snapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.sprints.len(), 1);
        assert_eq!(snapshot.sprints[0].prefix, Some("OLD".to_string()));
    }

    #[test]
    fn test_json_snapshot_loads_sprint_missing_card_prefix_as_none() {
        let json = r#"{
            "sprints": [{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "board_id": "550e8400-e29b-41d4-a716-446655440001",
                "sprint_number": 1,
                "name_index": null,
                "prefix": null,
                "status": "Planning",
                "start_date": null,
                "end_date": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let snapshot: Snapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.sprints.len(), 1);
        assert_eq!(snapshot.sprints[0].card_prefix, None);
    }

    #[test]
    fn test_json_sprint_deserialize_uses_record() {
        // A sprint object missing the required `id` field must error on load at
        // the SprintRecord boundary, not silently default. Compile-locks that
        // SprintRecord (not Sprint) owns the serde edge.
        let json = r#"{
            "sprints": [{
                "board_id": "550e8400-e29b-41d4-a716-446655440001",
                "sprint_number": 1,
                "name_index": null,
                "prefix": null,
                "status": "Planning",
                "start_date": null,
                "end_date": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let result: Result<Snapshot, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_column_create_store_load_equal_json() {
        use crate::column_factory::NewColumn;
        use uuid::Uuid;

        let now = "2024-03-03T00:00:00Z".parse().unwrap();
        let column = Column::create(
            NewColumn {
                board_id: Uuid::new_v4(),
                name: "Done".to_string(),
                wip_limit: Some(5),
                default_status: None,
            },
            Uuid::new_v4(),
            1,
            now,
        )
        .unwrap();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![column.clone()],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.columns.len(), 1);
        assert_eq!(restored.columns[0], column);
    }

    #[test]
    fn test_snapshot_archived_boards_defaults_when_absent_in_json() {
        let snap: Snapshot = serde_json::from_str(r#"{"boards": []}"#).unwrap();
        assert!(snap.archived_boards.is_empty());
    }

    #[test]
    fn test_snapshot_archived_boards_round_trips_wrapper() {
        let ab = crate::ArchivedBoard::now(Board::new("Archived", Some("KAN")).id);
        let mut snap = Snapshot::new();
        snap.archived_boards = vec![ab];

        let json = serde_json::to_string(&snap).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.archived_boards.len(), 1);
        assert_eq!(restored.archived_boards[0], ab);
    }

    #[test]
    fn test_snapshot_is_empty_false_when_only_archived_boards_present() {
        let mut snap = Snapshot::new();
        assert!(snap.is_empty());
        snap.archived_boards.push(crate::ArchivedBoard::now(
            Board::new("A", None::<String>).id,
        ));
        assert!(!snap.is_empty());
    }
}
