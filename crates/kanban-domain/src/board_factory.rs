use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::board::{
    default_next_sprint_number, default_sort_field, default_sort_order, Board, BoardId, SortField,
    SortOrder,
};
use crate::error::{KanbanError, KanbanResult};
use crate::task_list_view::TaskListView;

/// Client-settable CREATE fields only. Default-free. Never built with `..`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewBoard {
    pub name: String,
    pub description: Option<String>,
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    /// `None` => `Board::create` defaults to `SortField::Default`.
    pub task_sort_field: Option<SortField>,
    /// `None` => `SortOrder::Ascending`.
    pub task_sort_order: Option<SortOrder>,
    pub sprint_duration_days: Option<u32>,
    /// `None` => `TaskListView::default()`.
    pub task_list_view: Option<TaskListView>,
}

/// COMPLETE field set. The ONLY Board type deriving `Serialize`/`Deserialize`.
/// Default-free; exhaustively destructured in `reconstitute` + `From<&Board>`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoardRecord {
    pub id: BoardId,
    pub name: String,
    pub description: Option<String>,
    #[serde(default, alias = "branch_prefix")]
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub task_sort_field: SortField,
    pub task_sort_order: SortOrder,
    pub sprint_duration_days: Option<u32>,
    pub sprint_names: Vec<String>,
    pub sprint_name_used_count: usize,
    pub next_sprint_number: u32,
    pub active_sprint_id: Option<Uuid>,
    pub task_list_view: TaskListView,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl<'de> Deserialize<'de> for BoardRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct BoardHelper {
            pub id: BoardId,
            pub name: String,
            pub description: Option<String>,
            #[serde(default)]
            pub sprint_prefix: Option<String>,
            #[serde(default)]
            pub branch_prefix: Option<String>,
            #[serde(default)]
            pub card_prefix: Option<String>,
            #[serde(default = "default_sort_field")]
            pub task_sort_field: SortField,
            #[serde(default = "default_sort_order")]
            pub task_sort_order: SortOrder,
            #[serde(default)]
            pub sprint_duration_days: Option<u32>,
            #[serde(default)]
            pub sprint_names: Vec<String>,
            #[serde(default)]
            pub sprint_name_used_count: usize,
            #[serde(default = "default_next_sprint_number")]
            pub next_sprint_number: u32,
            #[serde(default)]
            pub active_sprint_id: Option<Uuid>,
            #[serde(default)]
            pub task_list_view: TaskListView,
            /// Pre-V12 key: parsed so a hand-rolled payload does not
            /// hard-fail, then discarded. The V12 migration is the supported
            /// upgrade path.
            #[serde(default, rename = "completion_column_id")]
            pub _completion_column_id: Option<Uuid>,
            /// Legacy key, superseded by `column.default_status`: parsed so a
            /// payload still carrying it does not hard-fail, then discarded.
            #[serde(default, rename = "completion_column_ids")]
            pub _completion_column_ids: Vec<Uuid>,
            #[serde(default)]
            pub position: i32,
            pub created_at: DateTime<Utc>,
            pub updated_at: DateTime<Utc>,
        }

        let helper = BoardHelper::deserialize(deserializer)?;
        let sprint_prefix = helper.sprint_prefix.or(helper.branch_prefix);

        Ok(BoardRecord {
            id: helper.id,
            name: helper.name,
            description: helper.description,
            sprint_prefix,
            card_prefix: helper.card_prefix,
            task_sort_field: helper.task_sort_field,
            task_sort_order: helper.task_sort_order,
            sprint_duration_days: helper.sprint_duration_days,
            sprint_names: helper.sprint_names,
            sprint_name_used_count: helper.sprint_name_used_count,
            next_sprint_number: helper.next_sprint_number,
            active_sprint_id: helper.active_sprint_id,
            task_list_view: helper.task_list_view,
            position: helper.position,
            created_at: helper.created_at,
            updated_at: helper.updated_at,
        })
    }
}

impl Board {
    /// Construct a brand-new board from client-supplied CREATE fields plus an
    /// injected id and clock. Server-managed counters are minted here; no
    /// internal `Uuid::new_v4`/`Utc::now`.
    pub fn create(spec: NewBoard, id: Uuid, now: DateTime<Utc>) -> KanbanResult<Board> {
        if spec.name.trim().is_empty() {
            return Err(KanbanError::validation("board name must not be blank"));
        }
        Ok(Board {
            id,
            name: spec.name,
            description: spec.description,
            sprint_prefix: spec.sprint_prefix,
            card_prefix: spec.card_prefix,
            task_sort_field: spec.task_sort_field.unwrap_or(SortField::Default),
            task_sort_order: spec.task_sort_order.unwrap_or(SortOrder::Ascending),
            sprint_duration_days: spec.sprint_duration_days,
            sprint_names: Vec::new(),
            sprint_name_used_count: 0,
            next_sprint_number: 1,
            active_sprint_id: None,
            task_list_view: spec.task_list_view.unwrap_or_default(),
            position: 0,
            created_at: now,
            updated_at: now,
        })
    }

    /// Rebuild a board from a persisted record. Structural validation only;
    /// invariants are assumed to have held when the record was written.
    pub fn reconstitute(rec: BoardRecord) -> KanbanResult<Board> {
        let BoardRecord {
            id,
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            sprint_names,
            sprint_name_used_count,
            next_sprint_number,
            active_sprint_id,
            task_list_view,
            position,
            created_at,
            updated_at,
        } = rec;
        // Legacy data may carry a blank board name (no validation existed before
        // the factory). Coerce to a placeholder on load rather than rejecting,
        // which would brick the board. `create` stays strict for new data.
        let name = if name.trim().is_empty() {
            "Untitled".to_string()
        } else {
            name
        };
        Ok(Board {
            id,
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            sprint_names,
            sprint_name_used_count,
            next_sprint_number,
            active_sprint_id,
            task_list_view,
            position,
            created_at,
            updated_at,
        })
    }
}

impl From<&Board> for BoardRecord {
    fn from(board: &Board) -> Self {
        let Board {
            id,
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            sprint_names,
            sprint_name_used_count,
            next_sprint_number,
            active_sprint_id,
            task_list_view,
            position,
            created_at,
            updated_at,
        } = board;
        BoardRecord {
            id: *id,
            name: name.clone(),
            description: description.clone(),
            sprint_prefix: sprint_prefix.clone(),
            card_prefix: card_prefix.clone(),
            task_sort_field: *task_sort_field,
            task_sort_order: *task_sort_order,
            sprint_duration_days: *sprint_duration_days,
            sprint_names: sprint_names.clone(),
            sprint_name_used_count: *sprint_name_used_count,
            next_sprint_number: *next_sprint_number,
            active_sprint_id: *active_sprint_id,
            task_list_view: *task_list_view,
            position: *position,
            created_at: *created_at,
            updated_at: *updated_at,
        }
    }
}

/// Serde adapter for a single `Board` field, routing bytes through
/// `BoardRecord` so construction always funnels through `Board::reconstitute`
/// (carrying the legacy V1..V7 migration) and serialization through the
/// `BoardRecord` decompose. Used via `#[serde(with = "board_serde")]`.
pub mod board_serde {
    use super::{Board, BoardRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(board: &Board, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BoardRecord::from(board).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Board, D::Error>
    where
        D: Deserializer<'de>,
    {
        let record = BoardRecord::deserialize(d)?;
        Board::reconstitute(record).map_err(serde::de::Error::custom)
    }
}

/// Serde adapter for a `Vec<Board>` field, routing every element through
/// `BoardRecord`. Used via `#[serde(with = "board_vec_serde")]`.
pub mod board_vec_serde {
    use super::{Board, BoardRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(boards: &[Board], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let records: Vec<BoardRecord> = boards.iter().map(BoardRecord::from).collect();
        records.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Vec<Board>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let records = Vec::<BoardRecord>::deserialize(d)?;
        records
            .into_iter()
            .map(Board::reconstitute)
            .collect::<crate::error::KanbanResult<Vec<Board>>>()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;

    #[test]
    fn test_board_record_ignores_legacy_completion_column_id_key() {
        // A hand-rolled pre-V12 payload reaching the deserialiser directly
        // must not hard-fail, and the legacy key must be discarded rather
        // than honoured. The V12 migration is the supported upgrade path.
        let json = format!(
            r#"{{
                "id": "{}",
                "name": "Legacy",
                "description": null,
                "completion_column_id": "{}",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }}"#,
            Uuid::new_v4(),
            Uuid::new_v4()
        );
        let rec: Result<BoardRecord, _> = serde_json::from_str(&json);
        assert!(
            rec.is_ok(),
            "a legacy completion_column_id key must not hard-fail deserialisation"
        );
    }

    #[test]
    fn test_board_record_ignores_a_legacy_completion_column_ids_field() {
        let json = format!(
            r#"{{
                "id": "{}",
                "name": "Legacy",
                "description": null,
                "completion_column_ids": ["{}"],
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }}"#,
            Uuid::new_v4(),
            Uuid::new_v4()
        );
        let rec: Result<BoardRecord, _> = serde_json::from_str(&json);
        assert!(
            rec.is_ok(),
            "a legacy completion_column_ids key must not hard-fail deserialisation"
        );
    }

    fn full_spec() -> NewBoard {
        NewBoard {
            name: "B".to_string(),
            description: Some("desc".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: Some(14),
            task_list_view: None,
        }
    }

    #[test]
    fn test_create_seeds_server_managed_counters_to_defaults() -> KanbanResult<()> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let board = Board::create(full_spec(), id, now)?;
        assert_eq!(board.next_sprint_number, 1);
        assert_eq!(board.sprint_name_used_count, 0);
        assert_eq!(board.position, 0);
        assert_eq!(board.active_sprint_id, None);
        assert!(board.sprint_names.is_empty());
        Ok(())
    }

    #[test]
    fn test_create_uses_injected_id_and_clock_not_internal() -> KanbanResult<()> {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let now = "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let board = Board::create(full_spec(), id, now)?;
        assert_eq!(board.id, id);
        assert_eq!(board.created_at, now);
        assert_eq!(board.updated_at, now);
        Ok(())
    }

    #[test]
    fn test_create_applies_content_fields_verbatim() -> KanbanResult<()> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let spec = NewBoard {
            name: "Board".to_string(),
            description: Some("a description".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: Some(21),
            task_list_view: None,
        };
        let board = Board::create(spec, id, now)?;
        assert_eq!(board.name, "Board");
        assert_eq!(board.description, Some("a description".to_string()));
        assert_eq!(board.sprint_prefix, Some("SPR".to_string()));
        assert_eq!(board.card_prefix, Some("KAN".to_string()));
        assert_eq!(board.sprint_duration_days, Some(21));
        Ok(())
    }

    #[test]
    fn test_create_defaults_omitted_sort_and_view_fields() -> KanbanResult<()> {
        let board = Board::create(full_spec(), Uuid::new_v4(), Utc::now())?;
        assert_eq!(board.task_sort_field, SortField::Default);
        assert_eq!(board.task_sort_order, SortOrder::Ascending);
        assert_eq!(board.task_list_view, TaskListView::default());
        Ok(())
    }

    #[test]
    fn test_create_honors_explicit_sort_and_view_fields() -> KanbanResult<()> {
        let spec = NewBoard {
            name: "B".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: Some(SortField::DueDate),
            task_sort_order: Some(SortOrder::Descending),
            sprint_duration_days: None,
            task_list_view: Some(TaskListView::ColumnView),
        };
        let board = Board::create(spec, Uuid::new_v4(), Utc::now())?;
        assert_eq!(board.task_sort_field, SortField::DueDate);
        assert_eq!(board.task_sort_order, SortOrder::Descending);
        assert_eq!(board.task_list_view, TaskListView::ColumnView);
        Ok(())
    }

    #[test]
    fn test_create_rejects_blank_name_returns_validation_error() {
        let spec = NewBoard {
            name: "  ".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
        };
        let err = Board::create(spec, Uuid::new_v4(), Utc::now()).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_new_board_spec_has_no_server_managed_fields() -> KanbanResult<()> {
        // NewBoard cannot set id/counters: the struct has no such fields, so an
        // attempt to set a next_sprint_number field on NewBoard would not compile.
        // Positive assertion: created counters are minted by `create`, not passed.
        let board = Board::create(full_spec(), Uuid::new_v4(), Utc::now())?;
        assert_eq!(board.next_sprint_number, 1);
        Ok(())
    }

    fn populated_record() -> BoardRecord {
        BoardRecord {
            id: Uuid::new_v4(),
            name: "Persisted".to_string(),
            description: Some("d".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: SortField::Priority,
            task_sort_order: SortOrder::Descending,
            sprint_duration_days: Some(14),
            sprint_names: vec!["Alpha".to_string(), "Beta".to_string()],
            sprint_name_used_count: 1,
            next_sprint_number: 9,
            active_sprint_id: Some(Uuid::new_v4()),
            task_list_view: TaskListView::GroupedByColumn,
            position: 3,
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn test_reconstitute_restores_every_field_verbatim() -> KanbanResult<()> {
        let rec = populated_record();
        let expected = rec.clone();
        let board = Board::reconstitute(rec)?;
        assert_eq!(board.id, expected.id);
        assert_eq!(board.name, expected.name);
        assert_eq!(board.description, expected.description);
        assert_eq!(board.sprint_prefix, expected.sprint_prefix);
        assert_eq!(board.card_prefix, expected.card_prefix);
        assert_eq!(board.task_sort_field, expected.task_sort_field);
        assert_eq!(board.task_sort_order, expected.task_sort_order);
        assert_eq!(board.sprint_duration_days, expected.sprint_duration_days);
        assert_eq!(board.sprint_names, expected.sprint_names);
        assert_eq!(
            board.sprint_name_used_count,
            expected.sprint_name_used_count
        );
        assert_eq!(board.next_sprint_number, expected.next_sprint_number);
        assert_eq!(board.active_sprint_id, expected.active_sprint_id);
        assert_eq!(board.task_list_view, expected.task_list_view);
        assert_eq!(board.position, expected.position);
        assert_eq!(board.created_at, expected.created_at);
        assert_eq!(board.updated_at, expected.updated_at);
        Ok(())
    }

    #[test]
    fn test_reconstitute_coerces_blank_name_to_placeholder() -> KanbanResult<()> {
        // Loading legacy data with a blank board name must not brick the board:
        // reconstitute coerces it to a placeholder instead of rejecting.
        let mut rec = populated_record();
        rec.name = "   ".to_string();
        let board = Board::reconstitute(rec)?;
        assert_eq!(board.name, "Untitled");
        Ok(())
    }

    #[test]
    fn test_decompose_then_reconstitute_is_identity() -> KanbanResult<()> {
        let board = Board::reconstitute(populated_record())?;
        let r = BoardRecord::from(&board);
        let b2 = Board::reconstitute(r)?;
        assert_eq!(board, b2);
        Ok(())
    }

    #[test]
    fn test_board_record_deserialize_branch_prefix_alias_maps_to_sprint_prefix() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "Test Board",
            "description": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "branch_prefix": "X",
            "card_prefix": null,
            "task_sort_field": "Default",
            "task_sort_order": "Ascending",
            "active_sprint_id": null,
            "sprint_duration_days": null,
            "sprint_names": [],
            "next_sprint_number": 1,
            "sprint_name_used_count": 0,
            "task_list_view": "Flat"
        }"#;
        let rec: BoardRecord = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(rec.sprint_prefix, Some("X".to_string()));
    }
}
