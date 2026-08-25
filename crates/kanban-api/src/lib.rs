//! HTTP API wire types shared by `kanban-server` and the http_backend transport.
//!
//! `v1` is private — canonical imports are from `kanban_service::api::*` directly.
//! This allows the versioning strategy to evolve (v2, v3, …) without locking
//! callers into an explicit version path.
mod v1;
pub use v1::{
    ApiError, ArchivedFilterDto, BoardResponse, CardGraphResponse, CardPriorityDto, CardResponse,
    CardStatusDto, ChangeEventFrame, ChangeKind, ColumnResponse, CreateBoardRequest,
    CreateCardRequest, CreateColumnRequest, CreateSprintParts, CreateSprintRequest, EntityType,
    ErrorCode, Page, PageParams, Patch, PrefixResponse, ReorderColumnRequest, ReplaceBoardRequest,
    ReplaceCardRequest, ReplaceColumnRequest, ReplaceSprintRequest, SortFieldDto, SortOrderDto,
    SprintResponse, SprintStatusDto, TaskListViewDto, UpdateBoardRequest, UpdateCardRequest,
    UpdateColumnRequest, UpdateSprintRequest,
};
