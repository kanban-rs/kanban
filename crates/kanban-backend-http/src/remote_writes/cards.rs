//! `RemoteWrites` card methods for `HttpBackend`.

use crate::conversions::card_from_response;
use crate::conversions_out::{create_card_request, update_card_request};
use crate::HttpBackend;
use kanban_api::{CardResponse, DeleteResponse, MutationResponse};
use kanban_domain::{Card, CardUpdate, Invalidation, KanbanResult, NewCard};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
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
}
