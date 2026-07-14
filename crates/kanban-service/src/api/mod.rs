//! HTTP API wire types shared by `kanban-server` and the http_backend transport.
//!
//! `v1` is private — canonical imports are from `kanban_service::api::*` directly.
//! This allows the versioning strategy to evolve (v2, v3, …) without locking
//! callers into an explicit version path.
mod v1;
pub use v1::{
    ApiError, ArchivedBoardResponse, ArchivedCardResponse, BoardResponse, CardPriorityDto,
    CardResponse, CardStatusDto, ChangeEventFrame, ColumnResponse, CreateBoardRequest,
    CreateCardRequest, CreateColumnRequest, CreateSprintParts, CreateSprintRequest, ErrorCode,
    Page, PageParams, Patch, ReorderColumnRequest, ReplaceBoardRequest, ReplaceCardRequest,
    ReplaceColumnRequest, ReplaceSprintRequest, SortFieldDto, SortOrderDto, SprintResponse,
    SprintStatusDto, TaskListViewDto, UpdateBoardRequest, UpdateCardRequest, UpdateColumnRequest,
    UpdateSprintRequest,
};
