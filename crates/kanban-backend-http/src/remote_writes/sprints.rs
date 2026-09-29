//! `RemoteWrites` sprint methods for `HttpBackend`.

use crate::conversions::sprint_from_response;
use crate::conversions_out::{create_sprint_request, update_sprint_request};
use crate::HttpBackend;
use kanban_api::{BoardResponse, DeleteResponse, MutationResponse, SprintResponse};
use kanban_domain::{Invalidation, KanbanResult, Sprint, SprintUpdate};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
    pub(crate) async fn sprint_with_pool(&self, resp: &SprintResponse) -> KanbanResult<Sprint> {
        let board: Option<BoardResponse> = self
            .get_json(&format!("/v1/boards/{}", resp.board_id))
            .await?;
        let sprint_names = board.map(|b| b.sprint_names).unwrap_or_else(|| {
            tracing::warn!(
                board_id = %resp.board_id,
                "board vanished between sprint and board reads; sprint will render unnamed"
            );
            Vec::new()
        });
        Ok(sprint_from_response(resp, &sprint_names))
    }

    pub(crate) fn rw_create_sprint(
        &self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<&str>,
        prefix: Option<&str>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        let req = create_sprint_request(id, name, prefix);
        self.block_on(async {
            let resp: MutationResponse<SprintResponse> = self
                .send_json_mutation(
                    Method::POST,
                    &format!("/v1/boards/{board_id}/sprints"),
                    Some(&req),
                )
                .await?;
            let sprint = self.sprint_with_pool(&resp.entity).await?;
            Ok((sprint, Invalidation::from(&resp.invalidation)))
        })
    }

    pub(crate) fn rw_update_sprint(
        &self,
        id: Uuid,
        updates: &SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        let req = update_sprint_request(updates)?;
        self.block_on(async {
            let resp: MutationResponse<SprintResponse> = self
                .send_json_mutation(Method::PATCH, &format!("/v1/sprints/{id}"), Some(&req))
                .await?;
            let sprint = self.sprint_with_pool(&resp.entity).await?;
            Ok((sprint, Invalidation::from(&resp.invalidation)))
        })
    }

    pub(crate) fn rw_delete_sprint(&self, id: Uuid) -> KanbanResult<Invalidation> {
        let resp: DeleteResponse = self.block_on(self.send_json_mutation::<(), DeleteResponse>(
            Method::DELETE,
            &format!("/v1/sprints/{id}"),
            None,
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }
}
