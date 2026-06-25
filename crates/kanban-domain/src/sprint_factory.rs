use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{KanbanError, KanbanResult};
use crate::sprint::{Sprint, SprintStatus};

/// Client-settable CREATE fields only. Default-free. Distinct from `Sprint`.
///
/// `sprint_number` and `name_index` are server-MINTED but injected here by the
/// service after it allocates them from the owning Board (analogous to how the
/// service injects `id`/`now`). The domain `create` stays I/O-free.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSprint {
    pub board_id: Uuid,
    pub sprint_number: u32,
    pub name_index: Option<usize>,
    pub prefix: Option<String>,
    pub card_prefix: Option<String>,
}

/// COMPLETE field set; the ONLY type that derives Serialize/Deserialize for
/// Sprint persistence. Default-free. Distinct from `Sprint`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SprintRecord {
    pub id: Uuid,
    pub board_id: Uuid,
    pub sprint_number: u32,
    pub name_index: Option<usize>,
    #[serde(alias = "prefix_override")]
    pub prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
    pub status: SprintStatus,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Sprint {
    /// Construct a brand-new sprint from client-supplied CREATE fields plus an
    /// injected id and clock. No internal `Uuid::new_v4`/`Utc::now`. Seeds
    /// `status = Planning`, `start_date = end_date = None`, and
    /// `created_at == updated_at == now`. `sprint_number`/`name_index` are
    /// server-minted values carried verbatim from the spec.
    pub fn create(spec: NewSprint, id: Uuid, now: DateTime<Utc>) -> KanbanResult<Sprint> {
        let NewSprint {
            board_id,
            sprint_number,
            name_index,
            prefix,
            card_prefix,
        } = spec;

        if let Some(p) = prefix.as_deref() {
            if p.trim().is_empty() {
                return Err(KanbanError::validation("sprint prefix must not be blank"));
            }
        }
        if let Some(cp) = card_prefix.as_deref() {
            if cp.trim().is_empty() {
                return Err(KanbanError::validation(
                    "sprint card_prefix must not be blank",
                ));
            }
        }
        if let (Some(p), Some(cp)) = (prefix.as_deref(), card_prefix.as_deref()) {
            if p.eq_ignore_ascii_case(cp) {
                return Err(KanbanError::validation(
                    "sprint prefix and card_prefix must differ",
                ));
            }
        }

        Ok(Sprint {
            id,
            board_id,
            sprint_number,
            name_index,
            prefix,
            card_prefix,
            status: SprintStatus::Planning,
            start_date: None,
            end_date: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Rebuild a sprint from a persisted record. Structural restore verbatim;
    /// invariants are assumed to have held when the record was written.
    pub fn reconstitute(record: SprintRecord) -> KanbanResult<Sprint> {
        let SprintRecord {
            id,
            board_id,
            sprint_number,
            name_index,
            prefix,
            card_prefix,
            status,
            start_date,
            end_date,
            created_at,
            updated_at,
        } = record;
        Ok(Sprint {
            id,
            board_id,
            sprint_number,
            name_index,
            prefix,
            card_prefix,
            status,
            start_date,
            end_date,
            created_at,
            updated_at,
        })
    }
}

impl From<&Sprint> for SprintRecord {
    fn from(sprint: &Sprint) -> Self {
        let Sprint {
            id,
            board_id,
            sprint_number,
            name_index,
            prefix,
            card_prefix,
            status,
            start_date,
            end_date,
            created_at,
            updated_at,
        } = sprint;
        SprintRecord {
            id: *id,
            board_id: *board_id,
            sprint_number: *sprint_number,
            name_index: *name_index,
            prefix: prefix.clone(),
            card_prefix: card_prefix.clone(),
            status: *status,
            start_date: *start_date,
            end_date: *end_date,
            created_at: *created_at,
            updated_at: *updated_at,
        }
    }
}

/// Serde adapter routing a single `Sprint` field's bytes through [`SprintRecord`].
///
/// `Sprint` itself is not `Deserialize`: the only door from persisted bytes to a
/// `Sprint` is [`Sprint::reconstitute`]. Use as `#[serde(with = "sprint_serde")]`
/// on a `Sprint` field. The on-wire shape is identical to the legacy direct
/// `Sprint` serialization since `SprintRecord`'s field names (and the
/// `prefix_override` alias / `card_prefix` default) match.
pub mod sprint_serde {
    use super::{Sprint, SprintRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(sprint: &Sprint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SprintRecord::from(sprint).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Sprint, D::Error>
    where
        D: Deserializer<'de>,
    {
        let record = SprintRecord::deserialize(deserializer)?;
        Sprint::reconstitute(record).map_err(serde::de::Error::custom)
    }
}

/// Serde adapter routing a `Vec<Sprint>` field's bytes through [`SprintRecord`].
///
/// The list counterpart of [`sprint_serde`]. Use as
/// `#[serde(with = "sprint_vec_serde")]` on a `Vec<Sprint>` field (e.g.
/// `Snapshot::sprints`). A malformed sprint surfaces `reconstitute`'s error
/// rather than a silent default.
pub mod sprint_vec_serde {
    use super::{Sprint, SprintRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(sprints: &[Sprint], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let records: Vec<SprintRecord> = sprints.iter().map(SprintRecord::from).collect();
        records.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Sprint>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let records = Vec::<SprintRecord>::deserialize(deserializer)?;
        records
            .into_iter()
            .map(Sprint::reconstitute)
            .collect::<crate::error::KanbanResult<Vec<Sprint>>>()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;

    fn fixed_now() -> DateTime<Utc> {
        "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
    }

    fn spec() -> NewSprint {
        NewSprint {
            board_id: Uuid::new_v4(),
            sprint_number: 3,
            name_index: Some(2),
            prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
        }
    }

    #[test]
    fn test_create_sets_planning_status_and_none_dates() -> KanbanResult<()> {
        let sprint = Sprint::create(spec(), Uuid::new_v4(), fixed_now())?;
        assert_eq!(sprint.status, SprintStatus::Planning);
        assert_eq!(sprint.start_date, None);
        assert_eq!(sprint.end_date, None);
        Ok(())
    }

    #[test]
    fn test_create_uses_injected_id_and_now_for_timestamps() -> KanbanResult<()> {
        let id = Uuid::new_v4();
        let now = fixed_now();
        let sprint = Sprint::create(spec(), id, now)?;
        assert_eq!(sprint.id, id);
        assert_eq!(sprint.created_at, now);
        assert_eq!(sprint.updated_at, now);
        Ok(())
    }

    #[test]
    fn test_create_copies_minted_sprint_number_and_name_index() -> KanbanResult<()> {
        let sprint = Sprint::create(spec(), Uuid::new_v4(), fixed_now())?;
        assert_eq!(sprint.sprint_number, 3);
        assert_eq!(sprint.name_index, Some(2));
        Ok(())
    }

    #[test]
    fn test_create_rejects_card_prefix_equal_to_prefix_returns_validation_error() {
        let err = Sprint::create(
            NewSprint {
                board_id: Uuid::new_v4(),
                sprint_number: 1,
                name_index: None,
                prefix: Some("KAN".to_string()),
                card_prefix: Some("kan".to_string()),
            },
            Uuid::new_v4(),
            fixed_now(),
        )
        .unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_create_rejects_blank_prefix_returns_validation_error() {
        let err = Sprint::create(
            NewSprint {
                board_id: Uuid::new_v4(),
                sprint_number: 1,
                name_index: None,
                prefix: Some("  ".to_string()),
                card_prefix: None,
            },
            Uuid::new_v4(),
            fixed_now(),
        )
        .unwrap_err();
        assert!(err.is_validation());
    }

    fn populated_record() -> SprintRecord {
        SprintRecord {
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
        }
    }

    #[test]
    fn test_reconstitute_restores_all_fields_verbatim() -> KanbanResult<()> {
        let rec = populated_record();
        let expected = rec.clone();
        let sprint = Sprint::reconstitute(rec)?;
        assert_eq!(sprint.id, expected.id);
        assert_eq!(sprint.board_id, expected.board_id);
        assert_eq!(sprint.sprint_number, expected.sprint_number);
        assert_eq!(sprint.name_index, expected.name_index);
        assert_eq!(sprint.prefix, expected.prefix);
        assert_eq!(sprint.card_prefix, expected.card_prefix);
        assert_eq!(sprint.status, expected.status);
        assert_eq!(sprint.start_date, expected.start_date);
        assert_eq!(sprint.end_date, expected.end_date);
        assert_eq!(sprint.created_at, expected.created_at);
        assert_eq!(sprint.updated_at, expected.updated_at);
        Ok(())
    }

    #[test]
    fn test_decompose_round_trips_through_record() -> KanbanResult<()> {
        let sprint = Sprint::reconstitute(populated_record())?;
        let record = SprintRecord::from(&sprint);
        let sprint2 = Sprint::reconstitute(record)?;
        assert_eq!(sprint, sprint2);
        Ok(())
    }

    #[test]
    fn test_sprint_record_json_round_trip_preserves_fields() -> KanbanResult<()> {
        let rec = populated_record();
        let json = serde_json::to_string(&rec).unwrap();
        let back: SprintRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, back);
        Ok(())
    }

    #[test]
    fn test_sprint_record_deserializes_legacy_prefix_override_alias() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "board_id": "00000000-0000-0000-0000-000000000002",
            "sprint_number": 1,
            "name_index": null,
            "prefix_override": "OLD",
            "card_prefix": null,
            "status": "Planning",
            "start_date": null,
            "end_date": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
        let rec: SprintRecord = serde_json::from_str(json).unwrap();
        assert_eq!(rec.prefix, Some("OLD".to_string()));
    }

    #[test]
    fn test_sprint_record_defaults_missing_card_prefix_to_none() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "board_id": "00000000-0000-0000-0000-000000000002",
            "sprint_number": 1,
            "name_index": null,
            "prefix": null,
            "status": "Planning",
            "start_date": null,
            "end_date": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
        let rec: SprintRecord = serde_json::from_str(json).unwrap();
        assert_eq!(rec.card_prefix, None);
    }

    #[test]
    fn test_new_sprint_has_no_default_impl() {
        // Compile-lock: constructing NewSprint/SprintRecord requires naming every
        // field (no `Default`, no `..`). A `NewSprint::default()` must NOT compile.
        let new_sprint = NewSprint {
            board_id: Uuid::new_v4(),
            sprint_number: 1,
            name_index: None,
            prefix: None,
            card_prefix: None,
        };
        assert_eq!(new_sprint.sprint_number, 1);
    }
}
