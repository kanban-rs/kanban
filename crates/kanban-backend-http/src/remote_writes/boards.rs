//! `RemoteWrites` board methods for `HttpBackend`.

use crate::conversions::board_from_response;
use crate::conversions_out::{create_board_request, update_board_request};
use crate::HttpBackend;
use kanban_api::{BoardResponse, DeleteResponse, MutationResponse};
use kanban_domain::{Board, BoardUpdate, Invalidation, KanbanResult, NewBoard};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
    pub(crate) fn rw_create_board(
        &self,
        id: Option<Uuid>,
        spec: &NewBoard,
    ) -> KanbanResult<(Board, Invalidation)> {
        let req = create_board_request(id, spec);
        let resp: MutationResponse<BoardResponse> =
            self.block_on(self.send_json_mutation(Method::POST, "/v1/boards", Some(&req)))?;
        Ok((
            board_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_update_board(
        &self,
        id: Uuid,
        updates: &BoardUpdate,
    ) -> KanbanResult<(Board, Invalidation)> {
        let req = update_board_request(updates)?;
        let resp: MutationResponse<BoardResponse> = self.block_on(self.send_json_mutation(
            Method::PATCH,
            &format!("/v1/boards/{id}"),
            Some(&req),
        ))?;
        Ok((
            board_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_delete_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        let resp: DeleteResponse = self.block_on(self.send_json_mutation::<(), DeleteResponse>(
            Method::DELETE,
            &format!("/v1/boards/{id}"),
            None,
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }
}
