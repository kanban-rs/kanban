use crate::app::App;
use crate::components::sprint_assign_list::build_entries;
use kanban_domain::{BoardSortField, SortField, SprintStatus};
use ratatui::Frame;

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

pub trait SelectionDialog {
    fn title(&self) -> &str;
    fn get_current_selection(&self, app: &App) -> usize;
    fn options_count(&self, app: &App) -> usize;
    fn render(&self, app: &App, frame: &mut Frame);
}

pub struct PriorityDialog;

impl SelectionDialog for PriorityDialog {
    fn title(&self) -> &str {
        "Set Priority"
    }

    fn get_current_selection(&self, app: &App) -> usize {
        app.get_current_priority_selection_index()
    }

    fn options_count(&self, _app: &App) -> usize {
        4 // Low, Medium, High, Critical
    }

    fn render(&self, app: &App, frame: &mut Frame) {
        use crate::components::render_selection_popup_with_list_items;
        use crate::theme::*;
        use kanban_domain::CardPriority;
        use ratatui::widgets::ListItem;

        let priorities = [
            CardPriority::Low,
            CardPriority::Medium,
            CardPriority::High,
            CardPriority::Critical,
        ];

        let selected = app.dialog_input.priority_selection.get();

        let items: Vec<ListItem> = priorities
            .iter()
            .enumerate()
            .map(|(idx, priority)| {
                let style = if Some(idx) == selected {
                    bold_highlight()
                } else {
                    normal_text()
                };
                ListItem::new(format!("{:?}", priority)).style(style)
            })
            .collect();

        render_selection_popup_with_list_items(frame, "Set Priority", items, 30, 40);
    }
}

pub struct BulkPriorityDialog {
    pub count: usize,
}

impl SelectionDialog for BulkPriorityDialog {
    fn title(&self) -> &str {
        "Set Priority (Bulk)"
    }

    fn get_current_selection(&self, _app: &App) -> usize {
        0
    }

    fn options_count(&self, _app: &App) -> usize {
        4 // Low, Medium, High, Critical
    }

    fn render(&self, app: &App, frame: &mut Frame) {
        use crate::components::render_selection_popup_with_list_items;
        use crate::theme::*;
        use kanban_domain::CardPriority;
        use ratatui::widgets::ListItem;

        let priorities = [
            CardPriority::Low,
            CardPriority::Medium,
            CardPriority::High,
            CardPriority::Critical,
        ];

        let selected = app.dialog_input.priority_selection.get();

        let items: Vec<ListItem> = priorities
            .iter()
            .enumerate()
            .map(|(idx, priority)| {
                let style = if Some(idx) == selected {
                    bold_highlight()
                } else {
                    normal_text()
                };
                ListItem::new(format!("{:?}", priority)).style(style)
            })
            .collect();

        let title = format!("Set Priority ({} cards)", self.count);
        render_selection_popup_with_list_items(frame, &title, items, 35, 40);
    }
}

pub struct SortFieldDialog;

impl SelectionDialog for SortFieldDialog {
    fn title(&self) -> &str {
        "Order Tasks By"
    }

    fn get_current_selection(&self, app: &App) -> usize {
        app.get_current_sort_field_selection_index()
    }

    fn options_count(&self, _app: &App) -> usize {
        SORT_FIELD_POPUP_ORDER.len()
    }

    fn render(&self, app: &App, frame: &mut Frame) {
        use crate::components::render_selection_popup_with_lines;
        use kanban_domain::SortOrder;

        let active_idx = app.filter.current_sort_field.map(popup_index_of_sort_field);

        render_selection_popup_with_lines(
            frame,
            "Order Tasks By",
            Some("Select sort field:"),
            SORT_FIELD_POPUP_ORDER.iter(),
            |_idx, entry, _is_selected, is_active| {
                let (_field, label) = **entry;
                let order_indicator = if is_active {
                    match app.filter.current_sort_order {
                        Some(SortOrder::Ascending) => Some(" (↑)".to_string()),
                        Some(SortOrder::Descending) => Some(" (↓)".to_string()),
                        None => None,
                    }
                } else {
                    None
                };

                (label.to_string(), order_indicator)
            },
            app.filter.sort_field_selection.get(),
            active_idx,
            60,
            50,
        );
    }
}

/// Field picker for the PROJECTS panel sort — the board-side analogue of
/// [`SortFieldDialog`] (KAN-948). Same list-with-active-order-indicator layout,
/// but backed by [`BOARD_SORT_FIELD_POPUP_ORDER`] and the unified board-list
/// sort state on the model.
pub struct BoardSortFieldDialog;

impl SelectionDialog for BoardSortFieldDialog {
    fn title(&self) -> &str {
        "Order Projects By"
    }

    fn get_current_selection(&self, app: &App) -> usize {
        app.get_current_board_sort_field_selection_index()
    }

    fn options_count(&self, _app: &App) -> usize {
        BOARD_SORT_FIELD_POPUP_ORDER.len()
    }

    fn render(&self, app: &App, frame: &mut Frame) {
        use crate::components::render_selection_popup_with_lines;
        use kanban_domain::SortOrder;

        let want_archived = matches!(app.get_base_mode(), crate::app::AppMode::ArchivedBoardsView);
        let (active_field, active_order) = app.model.board_sort(want_archived);
        let active_idx = Some(popup_index_of_board_sort_field(active_field));

        render_selection_popup_with_lines(
            frame,
            "Order Projects By",
            Some("Select sort field:"),
            BOARD_SORT_FIELD_POPUP_ORDER.iter(),
            |_idx, entry, _is_selected, is_active| {
                let (_field, label) = **entry;
                let order_indicator = if is_active {
                    match active_order {
                        SortOrder::Ascending => Some(" (↑)".to_string()),
                        SortOrder::Descending => Some(" (↓)".to_string()),
                    }
                } else {
                    None
                };

                (label.to_string(), order_indicator)
            },
            app.filter.board_sort_field_selection.get(),
            active_idx,
            60,
            50,
        );
    }
}

pub struct CarryOverSprintDialog {
    pub card_count: usize,
}

impl SelectionDialog for CarryOverSprintDialog {
    fn title(&self) -> &str {
        "Carry Over to Sprint"
    }

    fn get_current_selection(&self, app: &App) -> usize {
        app.dialog_input
            .carry_over_sprint_selection
            .get()
            .unwrap_or(0)
    }

    fn options_count(&self, app: &App) -> usize {
        if let Some(board) = app.active_board() {
            app.model
                .sprints()
                .iter()
                .filter(|s| s.board_id == board.id && s.status == SprintStatus::Planning)
                .count()
        } else {
            0
        }
    }

    fn render(&self, app: &App, frame: &mut Frame) {
        use crate::components::centered_rect;
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::{Color, Style},
            text::{Line, Span},
            widgets::{Block, Borders, Clear, Paragraph},
        };

        let area = centered_rect(60, 50, frame.area());
        frame.render_widget(Clear, area);

        let title = format!("Carry Over to Sprint ({} cards)", self.card_count);
        let block = Block::default()
            .title(title.as_str())
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);

        let label =
            Paragraph::new("Select target sprint:").style(Style::default().fg(Color::Yellow));
        frame.render_widget(label, chunks[0]);

        let mut lines = vec![];

        if let Some(board) = app.active_board() {
            {
                let sprints = app.model.sprints();
                let planning_sprints: Vec<_> = sprints
                    .iter()
                    .filter(|s| s.board_id == board.id && s.status == SprintStatus::Planning)
                    .collect();

                for (idx, sprint) in planning_sprints.iter().enumerate() {
                    let is_selected =
                        app.dialog_input.carry_over_sprint_selection.get() == Some(idx);

                    let style = if is_selected {
                        Style::default().fg(Color::White).bg(Color::Blue)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let prefix = if is_selected { "> " } else { "  " };
                    let sprint_name = sprint.formatted_name(board, "sprint");

                    lines.push(Line::from(Span::styled(
                        format!("{}{}", prefix, sprint_name),
                        style,
                    )));
                }
            }
        }

        let list = Paragraph::new(lines);
        frame.render_widget(list, chunks[1]);
    }
}

pub struct SprintAssignDialog;

impl SelectionDialog for SprintAssignDialog {
    fn title(&self) -> &str {
        "Assign to Sprint"
    }

    fn get_current_selection(&self, app: &App) -> usize {
        app.get_current_sprint_selection_index()
    }

    fn options_count(&self, app: &App) -> usize {
        if let Some(board) = app.active_board() {
            let sprints = app.model.sprints();
            return build_entries(sprints, board.id, chrono::Utc::now()).len();
        }
        1
    }

    fn render(&self, app: &App, frame: &mut Frame) {
        use crate::components::centered_rect;
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::{Color, Style},
            widgets::{Block, Borders, Clear, Paragraph},
        };

        let area = centered_rect(60, 50, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::default()
            .title("Assign to Sprint")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);

        frame.render_widget(
            Paragraph::new("Select sprint:").style(Style::default().fg(Color::Yellow)),
            chunks[0],
        );

        let Some(board) = app.active_board() else {
            return;
        };
        app.dialog_input.assign_sprint_picker.render(
            frame,
            chunks[1],
            app.model.sprints(),
            board,
            chrono::Utc::now(),
        );
    }
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
}
