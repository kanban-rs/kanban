//! `RemoteWrites` column methods for `HttpBackend`.

use crate::conversions::column_from_response;
use crate::conversions_out::{create_column_request, update_column_request};
use crate::HttpBackend;
use kanban_api::{ColumnResponse, DeleteResponse, MutationResponse};
use kanban_domain::{Column, ColumnUpdate, Invalidation, KanbanResult, NewColumn};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
    pub(crate) fn rw_create_column(
        &self,
        board_id: Uuid,
        spec: &NewColumn,
    ) -> KanbanResult<(Column, Invalidation)> {
        let (path, req) = create_column_request(board_id, spec);
        let resp: MutationResponse<ColumnResponse> =
            self.block_on(self.send_json_mutation(Method::POST, &path, Some(&req)))?;
        Ok((
            column_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_update_column(
        &self,
        id: Uuid,
        updates: &ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)> {
        let req = update_column_request(updates);
        let resp: MutationResponse<ColumnResponse> = self.block_on(self.send_json_mutation(
            Method::PATCH,
            &format!("/v1/columns/{id}"),
            Some(&req),
        ))?;
        Ok((
            column_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_delete_column(&self, id: Uuid) -> KanbanResult<Invalidation> {
        let resp: DeleteResponse = self.block_on(self.send_json_mutation::<(), DeleteResponse>(
            Method::DELETE,
            &format!("/v1/columns/{id}"),
            None,
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }
}
