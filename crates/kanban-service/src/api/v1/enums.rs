//! Wire-side mirrors of the domain enums, decoupling the HTTP contract from the
//! domain (and from the domain's persistence serde). They serialize as
//! `snake_case` and convert to/from the domain enums via exhaustive `From` impls
//! — a renamed or added domain variant fails to compile here (the drift guard).

use kanban_domain::{
    ArchivedFilter, CardPriority, CardStatus, SortField, SortOrder, SprintStatus, TaskListView,
};
use serde::{Deserialize, Serialize};

/// Wire mirror of [`kanban_domain::SortField`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SortFieldDto {
    Points,
    Priority,
    CreatedAt,
    UpdatedAt,
    DueDate,
    Status,
    Position,
    Default,
}

impl From<SortField> for SortFieldDto {
    fn from(value: SortField) -> Self {
        match value {
            SortField::Points => Self::Points,
            SortField::Priority => Self::Priority,
            SortField::CreatedAt => Self::CreatedAt,
            SortField::UpdatedAt => Self::UpdatedAt,
            SortField::DueDate => Self::DueDate,
            SortField::Status => Self::Status,
            SortField::Position => Self::Position,
            SortField::Default => Self::Default,
        }
    }
}

impl From<SortFieldDto> for SortField {
    fn from(value: SortFieldDto) -> Self {
        match value {
            SortFieldDto::Points => Self::Points,
            SortFieldDto::Priority => Self::Priority,
            SortFieldDto::CreatedAt => Self::CreatedAt,
            SortFieldDto::UpdatedAt => Self::UpdatedAt,
            SortFieldDto::DueDate => Self::DueDate,
            SortFieldDto::Status => Self::Status,
            SortFieldDto::Position => Self::Position,
            SortFieldDto::Default => Self::Default,
        }
    }
}

/// Wire mirror of [`kanban_domain::SortOrder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SortOrderDto {
    Ascending,
    Descending,
}

impl From<SortOrder> for SortOrderDto {
    fn from(value: SortOrder) -> Self {
        match value {
            SortOrder::Ascending => Self::Ascending,
            SortOrder::Descending => Self::Descending,
        }
    }
}

impl From<SortOrderDto> for SortOrder {
    fn from(value: SortOrderDto) -> Self {
        match value {
            SortOrderDto::Ascending => Self::Ascending,
            SortOrderDto::Descending => Self::Descending,
        }
    }
}

/// Wire mirror of [`kanban_domain::TaskListView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TaskListViewDto {
    Flat,
    GroupedByColumn,
    ColumnView,
}

impl From<TaskListView> for TaskListViewDto {
    fn from(value: TaskListView) -> Self {
        match value {
            TaskListView::Flat => Self::Flat,
            TaskListView::GroupedByColumn => Self::GroupedByColumn,
            TaskListView::ColumnView => Self::ColumnView,
        }
    }
}

impl From<TaskListViewDto> for TaskListView {
    fn from(value: TaskListViewDto) -> Self {
        match value {
            TaskListViewDto::Flat => Self::Flat,
            TaskListViewDto::GroupedByColumn => Self::GroupedByColumn,
            TaskListViewDto::ColumnView => Self::ColumnView,
        }
    }
}

/// Wire mirror of [`kanban_domain::CardPriority`]. Matches the domain `Display`
/// tokens (`low`/`medium`/`high`/`critical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CardPriorityDto {
    Low,
    Medium,
    High,
    Critical,
}

impl From<CardPriority> for CardPriorityDto {
    fn from(value: CardPriority) -> Self {
        match value {
            CardPriority::Low => Self::Low,
            CardPriority::Medium => Self::Medium,
            CardPriority::High => Self::High,
            CardPriority::Critical => Self::Critical,
        }
    }
}

impl From<CardPriorityDto> for CardPriority {
    fn from(value: CardPriorityDto) -> Self {
        match value {
            CardPriorityDto::Low => Self::Low,
            CardPriorityDto::Medium => Self::Medium,
            CardPriorityDto::High => Self::High,
            CardPriorityDto::Critical => Self::Critical,
        }
    }
}

/// Wire mirror of [`kanban_domain::CardStatus`]. Matches the domain `Display`
/// tokens (`todo`/`in_progress`/`blocked`/`done`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CardStatusDto {
    Todo,
    InProgress,
    Blocked,
    Done,
}

impl From<CardStatus> for CardStatusDto {
    fn from(value: CardStatus) -> Self {
        match value {
            CardStatus::Todo => Self::Todo,
            CardStatus::InProgress => Self::InProgress,
            CardStatus::Blocked => Self::Blocked,
            CardStatus::Done => Self::Done,
        }
    }
}

impl From<CardStatusDto> for CardStatus {
    fn from(value: CardStatusDto) -> Self {
        match value {
            CardStatusDto::Todo => Self::Todo,
            CardStatusDto::InProgress => Self::InProgress,
            CardStatusDto::Blocked => Self::Blocked,
            CardStatusDto::Done => Self::Done,
        }
    }
}

/// Wire mirror of [`kanban_domain::SprintStatus`]. Read-only on the response
/// side: sprint lifecycle transitions go through dedicated
/// activate/complete/cancel endpoints, never a create/update DTO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SprintStatusDto {
    Planning,
    Active,
    Completed,
    Cancelled,
}

impl From<SprintStatus> for SprintStatusDto {
    fn from(value: SprintStatus) -> Self {
        match value {
            SprintStatus::Planning => Self::Planning,
            SprintStatus::Active => Self::Active,
            SprintStatus::Completed => Self::Completed,
            SprintStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<SprintStatusDto> for SprintStatus {
    fn from(value: SprintStatusDto) -> Self {
        match value {
            SprintStatusDto::Planning => Self::Planning,
            SprintStatusDto::Active => Self::Active,
            SprintStatusDto::Completed => Self::Completed,
            SprintStatusDto::Cancelled => Self::Cancelled,
        }
    }
}

/// Wire mirror of [`kanban_domain::ArchivedFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArchivedFilterDto {
    #[default]
    LiveOnly,
    ArchivedOnly,
    Include,
}

impl From<ArchivedFilterDto> for ArchivedFilter {
    fn from(value: ArchivedFilterDto) -> Self {
        match value {
            ArchivedFilterDto::LiveOnly => Self::LiveOnly,
            ArchivedFilterDto::ArchivedOnly => Self::ArchivedOnly,
            ArchivedFilterDto::Include => Self::Include,
        }
    }
}

impl From<ArchivedFilter> for ArchivedFilterDto {
    fn from(value: ArchivedFilter) -> Self {
        match value {
            ArchivedFilter::LiveOnly => Self::LiveOnly,
            ArchivedFilter::ArchivedOnly => Self::ArchivedOnly,
            ArchivedFilter::Include => Self::Include,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dto_enums_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&SortFieldDto::CreatedAt).unwrap(),
            "\"created_at\""
        );
        assert_eq!(
            serde_json::to_string(&SortFieldDto::DueDate).unwrap(),
            "\"due_date\""
        );
        assert_eq!(
            serde_json::to_string(&SortFieldDto::Priority).unwrap(),
            "\"priority\""
        );
        assert_eq!(
            serde_json::to_string(&SortOrderDto::Descending).unwrap(),
            "\"descending\""
        );
        assert_eq!(
            serde_json::to_string(&TaskListViewDto::GroupedByColumn).unwrap(),
            "\"grouped_by_column\""
        );
        assert_eq!(
            serde_json::to_string(&TaskListViewDto::ColumnView).unwrap(),
            "\"column_view\""
        );
    }

    #[test]
    fn test_sort_field_dto_round_trips_through_domain() {
        for dto in [
            SortFieldDto::Points,
            SortFieldDto::Priority,
            SortFieldDto::CreatedAt,
            SortFieldDto::UpdatedAt,
            SortFieldDto::DueDate,
            SortFieldDto::Status,
            SortFieldDto::Position,
            SortFieldDto::Default,
        ] {
            let domain: SortField = dto.into();
            assert_eq!(SortFieldDto::from(domain), dto);
        }
    }

    /// Guard: the archived-boards recency dimension (`BoardSortField::ArchivedAt`)
    /// is a SEPARATE domain type from the card `SortField`, so it can never be a
    /// card sort field nor round-trip through this DTO. This is a compile-time
    /// guarantee — `SortFieldDto` has no `ArchivedAt` variant and `SortField`
    /// has no `ArchivedAt` variant — and the serde map below documents the exact
    /// card-sort wire surface a client can send.
    #[test]
    fn test_sort_field_dto_wire_surface_excludes_board_only_recency() {
        // The complete set of card-sort wire values. `archived_at` is absent by
        // construction (no variant), so a client cannot persist a board whose
        // card `task_sort_field` sorts by archival recency.
        let wire: Vec<String> = [
            SortFieldDto::Points,
            SortFieldDto::Priority,
            SortFieldDto::CreatedAt,
            SortFieldDto::UpdatedAt,
            SortFieldDto::DueDate,
            SortFieldDto::Status,
            SortFieldDto::Position,
            SortFieldDto::Default,
        ]
        .iter()
        .map(|dto| serde_json::to_string(dto).unwrap())
        .collect();
        assert!(
            !wire.iter().any(|w| w.contains("archived_at")),
            "card-sort DTO must not expose a board-only recency dimension"
        );
    }

    #[test]
    fn test_sort_order_dto_round_trips_through_domain() {
        for dto in [SortOrderDto::Ascending, SortOrderDto::Descending] {
            let domain: SortOrder = dto.into();
            assert_eq!(SortOrderDto::from(domain), dto);
        }
    }

    #[test]
    fn test_task_list_view_dto_round_trips_through_domain() {
        for dto in [
            TaskListViewDto::Flat,
            TaskListViewDto::GroupedByColumn,
            TaskListViewDto::ColumnView,
        ] {
            let domain: TaskListView = dto.into();
            assert_eq!(TaskListViewDto::from(domain), dto);
        }
    }

    #[test]
    fn test_dto_deserializes_from_snake_case() {
        let f: SortFieldDto = serde_json::from_str("\"updated_at\"").unwrap();
        assert_eq!(f, SortFieldDto::UpdatedAt);
        let v: TaskListViewDto = serde_json::from_str("\"flat\"").unwrap();
        assert_eq!(v, TaskListViewDto::Flat);
    }

    #[test]
    fn test_archived_filter_dto_round_trips_through_domain() {
        for dto in [
            ArchivedFilterDto::LiveOnly,
            ArchivedFilterDto::ArchivedOnly,
            ArchivedFilterDto::Include,
        ] {
            let domain: ArchivedFilter = dto.into();
            assert_eq!(ArchivedFilterDto::from(domain), dto);
        }
    }

    #[test]
    fn test_archived_filter_dto_default_is_live_only() {
        assert_eq!(ArchivedFilterDto::default(), ArchivedFilterDto::LiveOnly);
    }

    #[test]
    fn test_card_priority_dto_serializes_snake_case_and_round_trips_through_domain() {
        assert_eq!(
            serde_json::to_string(&CardPriorityDto::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(
            serde_json::to_string(&CardPriorityDto::Critical).unwrap(),
            "\"critical\""
        );
        for dto in [
            CardPriorityDto::Low,
            CardPriorityDto::Medium,
            CardPriorityDto::High,
            CardPriorityDto::Critical,
        ] {
            let domain: CardPriority = dto.into();
            assert_eq!(CardPriorityDto::from(domain), dto);
        }
    }

    #[test]
    fn test_card_status_dto_serializes_snake_case_and_round_trips_through_domain() {
        assert_eq!(
            serde_json::to_string(&CardStatusDto::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(
            serde_json::to_string(&CardStatusDto::Todo).unwrap(),
            "\"todo\""
        );
        for dto in [
            CardStatusDto::Todo,
            CardStatusDto::InProgress,
            CardStatusDto::Blocked,
            CardStatusDto::Done,
        ] {
            let domain: CardStatus = dto.into();
            assert_eq!(CardStatusDto::from(domain), dto);
        }
    }

    #[test]
    fn test_sprint_status_dto_serializes_snake_case_and_round_trips_through_domain() {
        assert_eq!(
            serde_json::to_string(&SprintStatusDto::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&SprintStatusDto::Cancelled).unwrap(),
            "\"cancelled\""
        );
        for dto in [
            SprintStatusDto::Planning,
            SprintStatusDto::Active,
            SprintStatusDto::Completed,
            SprintStatusDto::Cancelled,
        ] {
            let domain: SprintStatus = dto.into();
            assert_eq!(SprintStatusDto::from(domain), dto);
        }
    }
}
