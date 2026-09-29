use crate::conversions_out::update_card_request;
use kanban_api::{BatchOperationResponse, BatchUpdateItem, BatchUpdateRequest};
use kanban_backend::RemoteBatchOutcome;
use kanban_domain::CardUpdate;
use uuid::Uuid;

pub(crate) fn batch_update_request(updates: &[(Uuid, CardUpdate)]) -> BatchUpdateRequest {
    BatchUpdateRequest {
        updates: updates
            .iter()
            .map(|(id, u)| BatchUpdateItem {
                id: *id,
                update: update_card_request(u),
            })
            .collect(),
    }
}

pub(crate) fn outcome_from_response(resp: &BatchOperationResponse) -> RemoteBatchOutcome {
    RemoteBatchOutcome {
        succeeded: resp.succeeded.clone(),
        failed: resp
            .failed
            .iter()
            .map(|f| (f.id, f.error.clone()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_api::BatchFailure;
    use kanban_domain::{FieldUpdate, Invalidation};

    #[test]
    fn test_outcome_from_response_preserves_order_and_failure_messages() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let resp = BatchOperationResponse::new(
            vec![a, b],
            vec![BatchFailure::new(c, "Card c not found".to_string())],
            &Invalidation::All,
        );

        let outcome = outcome_from_response(&resp);

        assert_eq!(outcome.succeeded, vec![a, b]);
        assert_eq!(outcome.failed, vec![(c, "Card c not found".to_string())]);
    }

    #[test]
    fn test_batch_update_request_maps_each_update_through_update_card_request() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let update1 = CardUpdate {
            title: Some("Renamed".to_string()),
            ..Default::default()
        };
        let update2 = CardUpdate {
            points: FieldUpdate::Set(3),
            ..Default::default()
        };
        let updates = vec![(id1, update1.clone()), (id2, update2.clone())];

        let req = batch_update_request(&updates);

        assert_eq!(req.updates.len(), 2);
        assert_eq!(req.updates[0].id, id1);
        assert_eq!(
            serde_json::to_value(&req.updates[0].update).unwrap(),
            serde_json::to_value(update_card_request(&update1)).unwrap()
        );
        assert_eq!(req.updates[1].id, id2);
        assert_eq!(
            serde_json::to_value(&req.updates[1].update).unwrap(),
            serde_json::to_value(update_card_request(&update2)).unwrap()
        );
    }
}
