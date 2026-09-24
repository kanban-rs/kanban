use crate::InvalidationDto;
use kanban_domain::Invalidation;
use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationResponse<T> {
    #[serde(flatten)]
    pub entity: T,
    pub invalidation: InvalidationDto,
}

impl<T> MutationResponse<T> {
    pub fn new(entity: T, invalidation: &Invalidation) -> Self {
        Self {
            entity,
            invalidation: InvalidationDto::from(invalidation),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteResponse {
    pub invalidation: InvalidationDto,
}

impl DeleteResponse {
    pub fn new(invalidation: &Invalidation) -> Self {
        Self {
            invalidation: InvalidationDto::from(invalidation),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::BoardResponse;
    use super::{DeleteResponse, MutationResponse};
    use kanban_domain::{Board, EntityIds, Invalidation};
    use std::collections::HashSet;

    #[test]
    fn test_mutation_response_flattens_entity_and_carries_invalidation() {
        let board = Board::new("B", Some("KAN"));
        let board_id = board.id;
        let resp = BoardResponse::from(&board);
        let invalidation = Invalidation::Entities(EntityIds {
            boards: HashSet::from([board_id]),
            ..Default::default()
        });

        let mutation = MutationResponse::new(resp.clone(), &invalidation);
        let value = serde_json::to_value(&mutation).unwrap();

        assert_eq!(value["id"], board_id.to_string());
        assert_eq!(value["name"], "B");
        assert_eq!(value["invalidation"]["scope"], "entities");
        assert_eq!(
            value["invalidation"]["entities"]["boards"][0],
            board_id.to_string()
        );

        let round_tripped: MutationResponse<BoardResponse> = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped.entity, resp);
    }

    #[test]
    fn test_mutation_response_body_still_parses_as_the_bare_entity_dto() {
        let board = Board::new("B", Some("KAN"));
        let resp = BoardResponse::from(&board);
        let invalidation = Invalidation::All;

        let mutation = MutationResponse::new(resp.clone(), &invalidation);
        let value = serde_json::to_value(&mutation).unwrap();

        let bare: BoardResponse = serde_json::from_value(value).unwrap();
        assert_eq!(bare, resp);
    }

    #[test]
    fn test_delete_response_carries_only_the_invalidation() {
        let delete_resp = DeleteResponse::new(&Invalidation::All);
        let value = serde_json::to_value(&delete_resp).unwrap();

        let obj = value.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert_eq!(value["invalidation"]["scope"], "all");

        let round_tripped: DeleteResponse = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, delete_resp);

        let empty: Result<DeleteResponse, _> = serde_json::from_value(serde_json::json!({}));
        assert!(empty.is_err());
    }
}
