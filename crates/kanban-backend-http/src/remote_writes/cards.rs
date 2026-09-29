//! `RemoteWrites` card methods for `HttpBackend`.

use crate::conversions::card_from_response;
use crate::conversions_out::{
    assign_card_to_sprint_path, create_card_request, move_card_path, update_card_request,
};
use crate::HttpBackend;
use kanban_api::{CardResponse, DeleteResponse, MutationResponse};
use kanban_domain::{Card, CardUpdate, Invalidation, KanbanResult, NewCard};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
    fn post_card_mutation(&self, path: &str) -> KanbanResult<(Card, Invalidation)> {
        let resp: MutationResponse<CardResponse> = self.block_on(
            self.send_json_mutation::<(), MutationResponse<CardResponse>>(Method::POST, path, None),
        )?;
        Ok((
            card_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_create_card(
        &self,
        id: Option<Uuid>,
        spec: &NewCard,
    ) -> KanbanResult<(Card, Invalidation)> {
        let (path, req) = create_card_request(id, spec);
        let resp: MutationResponse<CardResponse> =
            self.block_on(self.send_json_mutation(Method::POST, &path, Some(&req)))?;
        Ok((
            card_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_update_card(
        &self,
        id: Uuid,
        updates: &CardUpdate,
    ) -> KanbanResult<(Card, Invalidation)> {
        let req = update_card_request(updates);
        let resp: MutationResponse<CardResponse> = self.block_on(self.send_json_mutation(
            Method::PATCH,
            &format!("/v1/cards/{id}"),
            Some(&req),
        ))?;
        Ok((
            card_from_response(&resp.entity),
            Invalidation::from(&resp.invalidation),
        ))
    }

    pub(crate) fn rw_delete_card(&self, id: Uuid) -> KanbanResult<Invalidation> {
        let resp: DeleteResponse = self.block_on(self.send_json_mutation::<(), DeleteResponse>(
            Method::DELETE,
            &format!("/v1/cards/{id}"),
            None,
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_archive_card(&self, id: Uuid) -> KanbanResult<Invalidation> {
        let resp: MutationResponse<CardResponse> = self.block_on(
            self.send_json_mutation::<(), MutationResponse<CardResponse>>(
                Method::POST,
                &format!("/v1/cards/{id}/archive"),
                None,
            ),
        )?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_restore_card(
        &self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<(Card, Invalidation)> {
        let path = match column_id {
            Some(column_id) => format!("/v1/cards/{id}/restore?column_id={column_id}"),
            None => format!("/v1/cards/{id}/restore"),
        };
        self.post_card_mutation(&path)
    }

    pub(crate) fn rw_move_card(
        &self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.post_card_mutation(&move_card_path(id, column_id, position))
    }

    pub(crate) fn rw_assign_card_to_sprint(
        &self,
        id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.post_card_mutation(&assign_card_to_sprint_path(id, sprint_id))
    }

    pub(crate) fn rw_unassign_card_from_sprint(
        &self,
        id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.post_card_mutation(&format!("/v1/cards/{id}/unassign-sprint"))
    }
}
