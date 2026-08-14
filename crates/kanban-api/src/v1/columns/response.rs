use chrono::{DateTime, Utc};
use kanban_domain::Column;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for column reads. `Deserialize` is derived intentionally (for
/// test round-trips and client/consumer use), though the server only serializes it.
/// Ids are plain `Uuid`, decoupled from the domain id aliases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnResponse {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&Column> for ColumnResponse {
    fn from(c: &Column) -> Self {
        Self {
            id: c.id,
            board_id: c.board_id,
            name: c.name.clone(),
            position: c.position,
            wip_limit: c.wip_limit,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_response_from_column() {
        let board_id = Uuid::new_v4();
        let column = Column::new(board_id, "Doing", 1);
        let resp = ColumnResponse::from(&column);
        assert_eq!(resp.id, column.id);
        assert_eq!(resp.board_id, board_id);
        assert_eq!(resp.name, "Doing");
        assert_eq!(resp.position, 1);
        let back: ColumnResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn test_column_response_dto_round_trips_default_status() {
        let board_id = Uuid::new_v4();
        let mut column = Column::new(board_id, "Doing", 1);
        column.default_status = Some(kanban_domain::CardStatus::InProgress);

        let resp = ColumnResponse::from(&column);

        assert_eq!(
            resp.default_status,
            Some(super::super::super::enums::CardStatusDto::InProgress)
        );
        let back: ColumnResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(back, resp);
    }
}
