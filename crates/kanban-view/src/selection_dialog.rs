use kanban_domain::{BoardSortField, CardStatus, SortField};

/// The `Option<CardStatus>` popup a column's `default_status` is edited
/// through; the leading `None` entry is the "clear" row.
pub const DEFAULT_STATUS_POPUP_ORDER: &[(Option<CardStatus>, &str)] = &[
    (None, "(none)"),
    (Some(CardStatus::Todo), "Todo"),
    (Some(CardStatus::InProgress), "In Progress"),
    (Some(CardStatus::Blocked), "Blocked"),
    (Some(CardStatus::Done), "Done"),
];

pub fn popup_index_of_default_status(status: Option<CardStatus>) -> usize {
    DEFAULT_STATUS_POPUP_ORDER
        .iter()
        .position(|(s, _)| *s == status)
        .unwrap_or(0)
}

pub fn default_status_at_popup_index(index: usize) -> Option<Option<CardStatus>> {
    DEFAULT_STATUS_POPUP_ORDER.get(index).map(|(s, _)| *s)
}

pub fn default_status_label(status: Option<CardStatus>) -> &'static str {
    DEFAULT_STATUS_POPUP_ORDER
        .iter()
        .find(|(s, _)| *s == status)
        .map(|(_, label)| *label)
        .unwrap_or("(none)")
}

pub const SORT_FIELD_POPUP_ORDER: &[(SortField, &str)] = &[
    (SortField::Points, "Points"),
    (SortField::Priority, "Priority"),
    (SortField::CreatedAt, "Date Created"),
    (SortField::UpdatedAt, "Date Updated"),
    (SortField::Status, "Status"),
    (SortField::Position, "Position"),
    (SortField::Default, "Task Number"),
    (SortField::DueDate, "Due Date"),
];

/// Board-list sort dimensions offered by the projects-panel field picker
/// (KAN-948). Mirrors [`SORT_FIELD_POPUP_ORDER`] but with the board-specific
/// [`BoardSortField`]: `ArchivedAt` is labelled "Recency" (the trash/history
/// dimension) since the term is more meaningful than the raw field name.
pub const BOARD_SORT_FIELD_POPUP_ORDER: &[(BoardSortField, &str)] = &[
    (BoardSortField::Position, "Position"),
    (BoardSortField::Name, "Name"),
    (BoardSortField::CreatedAt, "Date Created"),
    (BoardSortField::ArchivedAt, "Recency"),
];

pub fn popup_index_of_board_sort_field(field: BoardSortField) -> usize {
    BOARD_SORT_FIELD_POPUP_ORDER
        .iter()
        .position(|(f, _)| *f == field)
        .unwrap_or(0)
}

pub fn board_sort_field_at_popup_index(index: usize) -> Option<BoardSortField> {
    BOARD_SORT_FIELD_POPUP_ORDER.get(index).map(|(f, _)| *f)
}

pub fn popup_index_of_sort_field(field: SortField) -> usize {
    SORT_FIELD_POPUP_ORDER
        .iter()
        .position(|(f, _)| *f == field)
        .unwrap_or(0)
}

pub fn sort_field_at_popup_index(index: usize) -> Option<SortField> {
    SORT_FIELD_POPUP_ORDER.get(index).map(|(f, _)| *f)
}

#[cfg(test)]
mod sort_field_popup_tests {
    use super::*;

    #[test]
    fn test_sort_field_popup_order_includes_due_date() {
        assert!(
            SORT_FIELD_POPUP_ORDER
                .iter()
                .any(|(f, _)| *f == SortField::DueDate),
            "popup must expose DueDate"
        );
    }

    #[test]
    fn test_popup_index_round_trip_for_every_variant() {
        let variants = [
            SortField::Points,
            SortField::Priority,
            SortField::CreatedAt,
            SortField::UpdatedAt,
            SortField::DueDate,
            SortField::Status,
            SortField::Position,
            SortField::Default,
        ];

        for v in variants {
            let idx = popup_index_of_sort_field(v);
            assert_eq!(
                sort_field_at_popup_index(idx),
                Some(v),
                "round-trip failed for {:?}",
                v
            );
        }
    }

    #[test]
    fn test_popup_labels_are_non_empty() {
        for (field, label) in SORT_FIELD_POPUP_ORDER {
            assert!(!label.is_empty(), "label for {:?} is empty", field);
        }
    }

    #[test]
    fn test_board_sort_picker_lists_position_name_created_recency() {
        // The board-sort field picker offers exactly Position, Name, Date
        // Created, and Recency (=ArchivedAt), in that order.
        let labels: Vec<&str> = BOARD_SORT_FIELD_POPUP_ORDER
            .iter()
            .map(|(_, label)| *label)
            .collect();
        assert_eq!(
            labels,
            vec!["Position", "Name", "Date Created", "Recency"],
            "board sort picker labels/order"
        );
        let fields: Vec<BoardSortField> = BOARD_SORT_FIELD_POPUP_ORDER
            .iter()
            .map(|(f, _)| *f)
            .collect();
        assert_eq!(
            fields,
            vec![
                BoardSortField::Position,
                BoardSortField::Name,
                BoardSortField::CreatedAt,
                BoardSortField::ArchivedAt,
            ],
            "Recency maps to the ArchivedAt board field"
        );
    }

    #[test]
    fn test_board_sort_popup_index_round_trip_for_every_variant() {
        for v in [
            BoardSortField::Position,
            BoardSortField::Name,
            BoardSortField::CreatedAt,
            BoardSortField::ArchivedAt,
        ] {
            let idx = popup_index_of_board_sort_field(v);
            assert_eq!(board_sort_field_at_popup_index(idx), Some(v));
        }
    }

    #[test]
    fn test_default_status_popup_index_round_trip_for_every_variant() {
        for v in [
            None,
            Some(CardStatus::Todo),
            Some(CardStatus::InProgress),
            Some(CardStatus::Blocked),
            Some(CardStatus::Done),
        ] {
            let idx = popup_index_of_default_status(v);
            assert_eq!(default_status_at_popup_index(idx), Some(v));
        }
    }

    #[test]
    fn test_default_status_popup_labels_are_non_empty() {
        for (status, label) in DEFAULT_STATUS_POPUP_ORDER {
            assert!(!label.is_empty(), "label for {:?} is empty", status);
        }
    }
}
