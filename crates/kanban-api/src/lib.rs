//! HTTP API wire types shared by `kanban-server` and the http_backend transport.
//!
//! `v1` is private — canonical imports are from `kanban_service::api::*` directly.
//! This allows the versioning strategy to evolve (v2, v3, …) without locking
//! callers into an explicit version path.
mod v1;
pub use v1::{
    ActivateSprintRequest, AddBlockRequest, AddRelatedRequest, ApiError, ArchivedBoardResponse,
    ArchivedCardResponse, ArchivedFilterDto, AttachChildrenRequest, BatchArchiveRequest,
    BatchAssignSprintRequest, BatchFailure, BatchMoveRequest, BatchOperationResponse,
    BatchUpdateItem, BatchUpdateRequest, BlockEdgeDto, BoardResponse, CardGraphResponse,
    CardPriorityDto, CardResponse, CardStatusDto, CarryOverRequest, CarryOverResponse,
    ChangeEventFrame, ChangeKind, ColumnResponse, CreateBoardRequest, CreateCardRequest,
    CreateColumnRequest, CreateSprintParts, CreateSprintRequest, DeleteResponse, EntityIdsDto,
    EntityType, ErrorCode, InvalidationDto, MutationResponse, Page, PageParams, Patch,
    PrefixResponse, RelatedEdgeDto, RelatesKindDto, ReorderColumnRequest, ReplaceBoardRequest,
    ReplaceCardRequest, ReplaceColumnRequest, ReplaceSprintRequest, SeverityDto, SortFieldDto,
    SortOrderDto, SprintResponse, SprintStatusDto, TaskListViewDto, UpdateBoardRequest,
    UpdateCardRequest, UpdateColumnRequest, UpdateSprintRequest, CLIENT_ID_HEADER,
};
