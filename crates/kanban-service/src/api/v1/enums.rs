//! Wire-side mirrors of the domain enums, decoupling the HTTP contract from the
//! domain (and from the domain's persistence serde). They serialize as
//! `snake_case` and convert to/from the domain enums via exhaustive `From` impls
//! — a renamed or added domain variant fails to compile here (the drift guard).

use kanban_domain::{SortField, SortOrder, TaskListView};
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
}
