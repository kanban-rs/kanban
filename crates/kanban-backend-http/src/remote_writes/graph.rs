//! `RemoteGraphWrites` for `HttpBackend`.

use crate::HttpBackend;
use kanban_api::{
    AddBlockRequest, AddRelatedRequest, AttachChildrenRequest, CardGraphResponse, DeleteResponse,
    DetachChildrenRequest, MutationResponse,
};
use kanban_domain::{Invalidation, KanbanResult, RelatesKind, Severity};
use reqwest::Method;
use uuid::Uuid;

impl HttpBackend {
    pub(crate) fn rw_attach_children(
        &self,
        parent: Uuid,
        children: &[Uuid],
    ) -> KanbanResult<Invalidation> {
        let req = AttachChildrenRequest {
            children: children.to_vec(),
        };
        let resp: MutationResponse<CardGraphResponse> = self.block_on(self.send_json_mutation(
            Method::POST,
            &format!("/v1/cards/{parent}/children"),
            Some(&req),
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_detach_children(
        &self,
        parent: Uuid,
        children: &[Uuid],
    ) -> KanbanResult<Invalidation> {
        let req = DetachChildrenRequest {
            children: children.to_vec(),
        };
        let resp: MutationResponse<CardGraphResponse> = self.block_on(self.send_json_mutation(
            Method::POST,
            &format!("/v1/cards/{parent}/children/detach"),
            Some(&req),
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_block(
        &self,
        blocker: Uuid,
        blocked: Uuid,
        severity: Severity,
    ) -> KanbanResult<Invalidation> {
        let req = AddBlockRequest {
            blocked,
            severity: severity.into(),
        };
        let resp: MutationResponse<CardGraphResponse> = self.block_on(self.send_json_mutation(
            Method::POST,
            &format!("/v1/cards/{blocker}/blocks"),
            Some(&req),
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_unblock(&self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation> {
        let resp: DeleteResponse = self.block_on(self.send_json_mutation::<(), DeleteResponse>(
            Method::DELETE,
            &format!("/v1/cards/{blocker}/blocks/{blocked}"),
            None,
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_relate(
        &self,
        a: Uuid,
        b: Uuid,
        kind: RelatesKind,
    ) -> KanbanResult<Invalidation> {
        let req = AddRelatedRequest {
            other: b,
            kind: kind.into(),
        };
        let resp: MutationResponse<CardGraphResponse> = self.block_on(self.send_json_mutation(
            Method::POST,
            &format!("/v1/cards/{a}/related"),
            Some(&req),
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }

    pub(crate) fn rw_dissociate(&self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation> {
        let resp: DeleteResponse = self.block_on(self.send_json_mutation::<(), DeleteResponse>(
            Method::DELETE,
            &format!("/v1/cards/{a}/related/{b}"),
            None,
        ))?;
        Ok(Invalidation::from(&resp.invalidation))
    }
}
