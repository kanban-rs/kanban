//! `RemoteBatchWrites` for `HttpBackend`.

use crate::conversions_out::{batch_update_request, outcome_from_response};
use crate::HttpBackend;
use kanban_api::{
    BatchArchiveRequest, BatchAssignSprintRequest, BatchMoveRequest, BatchOperationResponse,
};
use kanban_backend::RemoteBatchOutcome;
use kanban_domain::{CardUpdate, Invalidation, KanbanResult};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
    pub(crate) fn rw_archive_cards(
        &self,
        ids: &[Uuid],
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        let req = BatchArchiveRequest { ids: ids.to_vec() };
        let resp: BatchOperationResponse = self.block_on(self.send_json_mutation(
            Method::POST,
            "/v1/cards/batch/archive",
            Some(&req),
        ))?;
        Ok((
            outcome_from_response(&resp),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_move_cards(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        let req = BatchMoveRequest {
            ids: ids.to_vec(),
            column_id,
        };
        let resp: BatchOperationResponse = self.block_on(self.send_json_mutation(
            Method::POST,
            "/v1/cards/batch/move",
            Some(&req),
        ))?;
        Ok((
            outcome_from_response(&resp),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_assign_cards_to_sprint(
        &self,
        ids: &[Uuid],
        sprint_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        let req = BatchAssignSprintRequest {
            ids: ids.to_vec(),
            sprint_id,
        };
        let resp: BatchOperationResponse = self.block_on(self.send_json_mutation(
            Method::POST,
            "/v1/cards/batch/assign-sprint",
            Some(&req),
        ))?;
        Ok((
            outcome_from_response(&resp),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_update_cards(
        &self,
        updates: &[(Uuid, CardUpdate)],
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        let req = batch_update_request(updates);
        let resp: BatchOperationResponse = self.block_on(self.send_json_mutation(
            Method::POST,
            "/v1/cards/batch/update",
            Some(&req),
        ))?;
        Ok((
            outcome_from_response(&resp),
            Invalidation::from(&resp.invalidation),
        ))
    }
}
