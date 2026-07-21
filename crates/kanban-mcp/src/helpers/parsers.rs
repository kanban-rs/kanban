use kanban_core::parse_datetime_input;
use kanban_domain::{
    ArchivedFilter, BoardSortField, CardPriority, CardStatus, SortField, SortOrder,
};
use rmcp::model::ErrorData as McpError;
use std::str::FromStr;

/// Parse the `archived` list selector: `exclude` (live only, the default),
/// `only` (archived only), or `include` (both). Mirrors the three-state
/// `ArchivedFilter` the unified card list applies.
pub(crate) fn parse_archived_selector(s: &str) -> Result<ArchivedFilter, McpError> {
    match s.to_lowercase().as_str() {
        "exclude" | "live" => Ok(ArchivedFilter::LiveOnly),
        "only" | "archived" => Ok(ArchivedFilter::ArchivedOnly),
        "include" | "both" => Ok(ArchivedFilter::Include),
        _ => Err(McpError::invalid_params(
            format!("Invalid archived filter '{s}'. Valid: exclude, only, include"),
            None,
        )),
    }
}

pub(crate) fn parse_priority(s: &str) -> Result<CardPriority, McpError> {
    match s.to_lowercase().as_str() {
        "low" => Ok(CardPriority::Low),
        "medium" => Ok(CardPriority::Medium),
        "high" => Ok(CardPriority::High),
        "critical" => Ok(CardPriority::Critical),
        _ => Err(McpError::invalid_params(
            format!(
                "Invalid priority '{}'. Valid: low, medium, high, critical",
                s
            ),
            None,
        )),
    }
}

pub(crate) fn parse_status(s: &str) -> Result<CardStatus, McpError> {
    match s.to_lowercase().replace(['-', '_'], "").as_str() {
        "todo" => Ok(CardStatus::Todo),
        "inprogress" => Ok(CardStatus::InProgress),
        "blocked" => Ok(CardStatus::Blocked),
        "done" => Ok(CardStatus::Done),
        _ => Err(McpError::invalid_params(
            format!(
                "Invalid status '{}'. Valid: todo, in_progress, blocked, done",
                s
            ),
            None,
        )),
    }
}

pub(crate) fn parse_datetime(s: &str) -> Result<chrono::DateTime<chrono::Utc>, McpError> {
    parse_datetime_input(s).map_err(|msg| McpError::invalid_params(msg, None))
}

pub(crate) fn parse_sort_field(s: &str) -> Result<SortField, McpError> {
    match s.to_lowercase().replace(['-', '_'], "").as_str() {
        "points" => Ok(SortField::Points),
        "priority" => Ok(SortField::Priority),
        "createdat" => Ok(SortField::CreatedAt),
        "updatedat" => Ok(SortField::UpdatedAt),
        "duedate" => Ok(SortField::DueDate),
        "status" => Ok(SortField::Status),
        "position" => Ok(SortField::Position),
        "default" => Ok(SortField::Default),
        _ => Err(McpError::invalid_params(
            format!(
                "Invalid sort field '{}'. Valid: points, priority, created_at, updated_at, due_date, status, position, default",
                s
            ),
            None,
        )),
    }
}

/// Parse a board-list sort field via the canonical domain [`BoardSortField`]
/// `FromStr` (R1, KAN-950), mapping the unit parse error to an MCP
/// `invalid_params`. The MCP layer owns only the error-type mapping; the
/// accepted token set lives in the domain.
pub(crate) fn parse_board_sort_field(s: &str) -> Result<BoardSortField, McpError> {
    BoardSortField::from_str(s).map_err(|()| {
        McpError::invalid_params(
            format!(
                "Invalid board sort field '{s}'. Valid: position, name, created_at, archived_at"
            ),
            None,
        )
    })
}

/// Parse a sort order via the canonical domain [`SortOrder`] `FromStr` (R1,
/// KAN-950), mapping the unit parse error to an MCP `invalid_params`.
pub(crate) fn parse_sort_order(s: &str) -> Result<SortOrder, McpError> {
    SortOrder::from_str(s).map_err(|()| {
        McpError::invalid_params(format!("Invalid sort order '{s}'. Valid: asc, desc"), None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // parse_priority

    #[test]
    fn parse_priority_all_valid() {
        assert!(matches!(parse_priority("low").unwrap(), CardPriority::Low));
        assert!(matches!(
            parse_priority("medium").unwrap(),
            CardPriority::Medium
        ));
        assert!(matches!(
            parse_priority("high").unwrap(),
            CardPriority::High
        ));
        assert!(matches!(
            parse_priority("critical").unwrap(),
            CardPriority::Critical
        ));
    }

    #[test]
    fn parse_priority_case_insensitive() {
        assert!(matches!(parse_priority("LOW").unwrap(), CardPriority::Low));
        assert!(matches!(
            parse_priority("High").unwrap(),
            CardPriority::High
        ));
        assert!(matches!(
            parse_priority("CRITICAL").unwrap(),
            CardPriority::Critical
        ));
    }

    #[test]
    fn parse_priority_invalid() {
        let err = parse_priority("urgent").unwrap_err();
        assert!(err.message.contains("Invalid priority"));
    }

    // parse_archived_selector

    #[test]
    fn parse_archived_selector_maps_three_states() {
        use kanban_domain::ArchivedFilter;
        assert_eq!(
            parse_archived_selector("exclude").unwrap(),
            ArchivedFilter::LiveOnly
        );
        assert_eq!(
            parse_archived_selector("ONLY").unwrap(),
            ArchivedFilter::ArchivedOnly
        );
        assert_eq!(
            parse_archived_selector("Include").unwrap(),
            ArchivedFilter::Include
        );
    }

    #[test]
    fn parse_archived_selector_rejects_unknown() {
        assert!(parse_archived_selector("nope").is_err());
    }

    // parse_status

    #[test]
    fn parse_status_all_valid() {
        assert!(matches!(parse_status("todo").unwrap(), CardStatus::Todo));
        assert!(matches!(
            parse_status("in_progress").unwrap(),
            CardStatus::InProgress
        ));
        assert!(matches!(
            parse_status("blocked").unwrap(),
            CardStatus::Blocked
        ));
        assert!(matches!(parse_status("done").unwrap(), CardStatus::Done));
    }

    #[test]
    fn parse_status_hyphen_underscore_normalization() {
        assert!(matches!(
            parse_status("in-progress").unwrap(),
            CardStatus::InProgress
        ));
        assert!(matches!(
            parse_status("in_progress").unwrap(),
            CardStatus::InProgress
        ));
        assert!(matches!(
            parse_status("InProgress").unwrap(),
            CardStatus::InProgress
        ));
    }

    #[test]
    fn parse_status_invalid() {
        let err = parse_status("cancelled").unwrap_err();
        assert!(err.message.contains("Invalid status"));
    }

    // parse_sort_field

    #[test]
    fn parse_sort_field_accepts_due_date_and_kebab_case() {
        use kanban_domain::SortField;
        assert_eq!(parse_sort_field("due-date").unwrap(), SortField::DueDate);
        assert_eq!(parse_sort_field("due_date").unwrap(), SortField::DueDate);
        assert_eq!(parse_sort_field("DueDate").unwrap(), SortField::DueDate);
    }

    #[test]
    fn parse_sort_field_covers_every_variant() {
        use kanban_domain::SortField;
        assert_eq!(parse_sort_field("points").unwrap(), SortField::Points);
        assert_eq!(parse_sort_field("priority").unwrap(), SortField::Priority);
        assert_eq!(
            parse_sort_field("created-at").unwrap(),
            SortField::CreatedAt
        );
        assert_eq!(
            parse_sort_field("updated-at").unwrap(),
            SortField::UpdatedAt
        );
        assert_eq!(parse_sort_field("due-date").unwrap(), SortField::DueDate);
        assert_eq!(parse_sort_field("status").unwrap(), SortField::Status);
        assert_eq!(parse_sort_field("position").unwrap(), SortField::Position);
        assert_eq!(parse_sort_field("default").unwrap(), SortField::Default);
    }

    #[test]
    fn parse_sort_field_rejects_unknown() {
        let err = parse_sort_field("magnitude").unwrap_err();
        assert!(err.message.contains("Invalid sort field"));
    }

    // parse_board_sort_field

    #[test]
    fn parse_board_sort_field_covers_every_variant() {
        use kanban_domain::BoardSortField;
        assert_eq!(
            parse_board_sort_field("position").unwrap(),
            BoardSortField::Position
        );
        assert_eq!(
            parse_board_sort_field("name").unwrap(),
            BoardSortField::Name
        );
        assert_eq!(
            parse_board_sort_field("created-at").unwrap(),
            BoardSortField::CreatedAt
        );
        assert_eq!(
            parse_board_sort_field("archived_at").unwrap(),
            BoardSortField::ArchivedAt
        );
    }

    #[test]
    fn parse_board_sort_field_rejects_unknown() {
        let err = parse_board_sort_field("priority").unwrap_err();
        assert!(err.message.contains("Invalid board sort field"));
    }

    // parse_sort_order

    #[test]
    fn parse_sort_order_accepts_asc_and_desc() {
        use kanban_domain::SortOrder;
        assert_eq!(parse_sort_order("asc").unwrap(), SortOrder::Ascending);
        assert_eq!(parse_sort_order("ascending").unwrap(), SortOrder::Ascending);
        assert_eq!(parse_sort_order("desc").unwrap(), SortOrder::Descending);
        assert_eq!(
            parse_sort_order("Descending").unwrap(),
            SortOrder::Descending
        );
    }

    #[test]
    fn parse_sort_order_rejects_unknown() {
        let err = parse_sort_order("sideways").unwrap_err();
        assert!(err.message.contains("Invalid sort order"));
    }

    // parse_datetime

    #[test]
    fn parse_datetime_rfc3339() {
        let dt = parse_datetime("2024-06-15T10:30:00Z").unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-06-15T10:30:00+00:00");
    }

    #[test]
    fn parse_datetime_date_only() {
        let dt = parse_datetime("2024-06-15").unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-06-15T00:00:00+00:00");
    }

    #[test]
    fn parse_datetime_invalid() {
        let err = parse_datetime("not-a-date").unwrap_err();
        assert!(err.message.contains("Invalid date"));
    }
}
