use chrono::{DateTime, Utc};

use crate::board::BoardId;
use crate::column::{Column, ColumnId};
use crate::error::{KanbanError, KanbanResult};

/// Client-settable CREATE fields only. Default-free. Never built with `..`.
///
/// `board_id` is the FK the column is created against (data); FK existence is
/// validated at the service tier, not here. `position` is NOT a field here:
/// the server assigns the append index, passed to `Column::create`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewColumn {
    pub board_id: BoardId,
    pub name: String,
    pub wip_limit: Option<i32>,
}

/// COMPLETE field set. The ONLY Column type deriving `Serialize`/`Deserialize`
/// for persistence. Default-free, distinct from `Column`; exhaustively
/// destructured in `reconstitute` + `From<&Column>`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColumnRecord {
    pub id: ColumnId,
    pub board_id: BoardId,
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Column {
    /// Construct a brand-new column from client-supplied CREATE fields plus an
    /// injected id, server-assigned append position, and clock. No internal
    /// `Uuid::new_v4`/`Utc::now`. Enforces the non-negativity invariants on
    /// `position` and `wip_limit`.
    pub fn create(
        spec: NewColumn,
        id: ColumnId,
        position: i32,
        now: DateTime<Utc>,
    ) -> KanbanResult<Column> {
        let NewColumn {
            board_id,
            name,
            wip_limit,
        } = spec;
        if name.trim().is_empty() {
            return Err(KanbanError::validation("column name must not be blank"));
        }
        if position < 0 {
            return Err(KanbanError::validation(
                "column position must not be negative",
            ));
        }
        if let Some(limit) = wip_limit {
            if limit < 0 {
                return Err(KanbanError::validation(
                    "column wip_limit must not be negative",
                ));
            }
        }
        Ok(Column {
            id,
            board_id,
            name,
            position,
            wip_limit,
            created_at: now,
            updated_at: now,
        })
    }

    /// Rebuild a column from a persisted record. Structural validation only;
    /// invariants are assumed to have held when the record was written.
    pub fn reconstitute(record: ColumnRecord) -> KanbanResult<Column> {
        let ColumnRecord {
            id,
            board_id,
            name,
            position,
            wip_limit,
            created_at,
            updated_at,
        } = record;
        // Legacy data may carry a blank column name (no validation existed before
        // the factory). Coerce to a placeholder on load rather than rejecting,
        // which would brick the whole board. `create` stays strict for new data.
        let name = if name.trim().is_empty() {
            "Untitled".to_string()
        } else {
            name
        };
        Ok(Column {
            id,
            board_id,
            name,
            position,
            wip_limit,
            created_at,
            updated_at,
        })
    }
}

impl From<&Column> for ColumnRecord {
    fn from(column: &Column) -> Self {
        let Column {
            id,
            board_id,
            name,
            position,
            wip_limit,
            created_at,
            updated_at,
        } = column;
        ColumnRecord {
            id: *id,
            board_id: *board_id,
            name: name.clone(),
            position: *position,
            wip_limit: *wip_limit,
            created_at: *created_at,
            updated_at: *updated_at,
        }
    }
}

/// Serde adapter for a single `Column` field, routing bytes through
/// `ColumnRecord` so construction always funnels through `Column::reconstitute`
/// and serialization through the `ColumnRecord` decompose. Used via
/// `#[serde(with = "column_serde")]`.
pub mod column_serde {
    use super::{Column, ColumnRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(column: &Column, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ColumnRecord::from(column).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Column, D::Error>
    where
        D: Deserializer<'de>,
    {
        let record = ColumnRecord::deserialize(d)?;
        Column::reconstitute(record).map_err(serde::de::Error::custom)
    }
}

/// Serde adapter for a `Vec<Column>` field, routing every element through
/// `ColumnRecord`. Used via `#[serde(with = "column_vec_serde")]`.
pub mod column_vec_serde {
    use super::{Column, ColumnRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(columns: &[Column], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let records: Vec<ColumnRecord> = columns.iter().map(ColumnRecord::from).collect();
        records.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Vec<Column>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let records = Vec::<ColumnRecord>::deserialize(d)?;
        records
            .into_iter()
            .map(Column::reconstitute)
            .collect::<crate::error::KanbanResult<Vec<Column>>>()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;
    use uuid::Uuid;

    fn spec(wip_limit: Option<i32>) -> NewColumn {
        NewColumn {
            board_id: Uuid::new_v4(),
            name: "To Do".to_string(),
            wip_limit,
        }
    }

    #[test]
    fn test_create_sets_fields_from_spec_and_injected_id_clock() -> KanbanResult<()> {
        let board_id = Uuid::new_v4();
        let id = Uuid::new_v4();
        let now = "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let column = Column::create(
            NewColumn {
                board_id,
                name: "To Do".to_string(),
                wip_limit: None,
            },
            id,
            0,
            now,
        )?;
        assert_eq!(column.id, id);
        assert_eq!(column.board_id, board_id);
        assert_eq!(column.name, "To Do");
        assert_eq!(column.position, 0);
        assert_eq!(column.created_at, now);
        assert_eq!(column.updated_at, now);
        Ok(())
    }

    #[test]
    fn test_create_carries_wip_limit_from_spec() -> KanbanResult<()> {
        let column = Column::create(spec(Some(3)), Uuid::new_v4(), 0, Utc::now())?;
        assert_eq!(column.wip_limit, Some(3));
        Ok(())
    }

    #[test]
    fn test_create_defaults_wip_limit_to_none_when_absent() -> KanbanResult<()> {
        let column = Column::create(spec(None), Uuid::new_v4(), 0, Utc::now())?;
        assert_eq!(column.wip_limit, None);
        Ok(())
    }

    #[test]
    fn test_create_rejects_negative_position_returns_validation_error() {
        let err = Column::create(spec(None), Uuid::new_v4(), -1, Utc::now()).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_create_rejects_negative_wip_limit_returns_validation_error() {
        let err = Column::create(
            NewColumn {
                board_id: Uuid::new_v4(),
                name: "To Do".to_string(),
                wip_limit: Some(-1),
            },
            Uuid::new_v4(),
            0,
            Utc::now(),
        )
        .unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_create_rejects_blank_name_returns_validation_error() {
        let err = Column::create(
            NewColumn {
                board_id: Uuid::new_v4(),
                name: "   ".to_string(),
                wip_limit: None,
            },
            Uuid::new_v4(),
            0,
            Utc::now(),
        )
        .unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_reconstitute_coerces_blank_name_to_placeholder() -> KanbanResult<()> {
        // Loading legacy data with a blank column name must not brick the board:
        // reconstitute coerces it to a placeholder instead of rejecting.
        let mut rec = populated_record();
        rec.name = "   ".to_string();
        let column = Column::reconstitute(rec)?;
        assert_eq!(column.name, "Untitled");
        Ok(())
    }

    #[test]
    fn test_create_does_not_read_system_clock() -> KanbanResult<()> {
        let now = "2024-03-03T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let a = Column::create(spec(None), Uuid::new_v4(), 0, now)?;
        let b = Column::create(spec(None), Uuid::new_v4(), 0, now)?;
        assert_eq!(a.created_at, b.created_at);
        assert_eq!(a.created_at, now);
        Ok(())
    }

    fn populated_record() -> ColumnRecord {
        ColumnRecord {
            id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            name: "In Progress".to_string(),
            position: 2,
            wip_limit: Some(5),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn test_reconstitute_restores_record_verbatim() -> KanbanResult<()> {
        let rec = populated_record();
        let expected = rec.clone();
        let column = Column::reconstitute(rec)?;
        assert_eq!(column.id, expected.id);
        assert_eq!(column.board_id, expected.board_id);
        assert_eq!(column.name, expected.name);
        assert_eq!(column.position, expected.position);
        assert_eq!(column.wip_limit, expected.wip_limit);
        assert_eq!(column.created_at, expected.created_at);
        assert_eq!(column.updated_at, expected.updated_at);
        assert_ne!(column.created_at, column.updated_at);
        Ok(())
    }

    #[test]
    fn test_decompose_round_trip_column_to_record_to_column() -> KanbanResult<()> {
        let column = Column::reconstitute(populated_record())?;
        let record = ColumnRecord::from(&column);
        let column2 = Column::reconstitute(record)?;
        assert_eq!(column, column2);
        Ok(())
    }

    #[test]
    fn test_new_column_is_default_free() {
        // Compile-lock: constructing NewColumn/ColumnRecord requires naming every
        // field (no `Default`, no `..`). If a Default impl crept in, this would
        // still compile, but the CI grep guard from the Foundation card forbids it.
        let new_column = NewColumn {
            board_id: Uuid::new_v4(),
            name: "Done".to_string(),
            wip_limit: Some(0),
        };
        assert_eq!(new_column.name, "Done");
        let record = ColumnRecord {
            id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            name: "Done".to_string(),
            position: 0,
            wip_limit: Some(0),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(record.position, 0);
    }
}
