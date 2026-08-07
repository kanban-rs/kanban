use kanban_domain::Board;
use kanban_view::sprint_assign_list::SprintAssignEntry;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use uuid::Uuid;

/// Renders a single dialog row for the given entry. Shared by both the
/// single-card and multi-card sprint-assign dialogs. Pass
/// `current_sprint_id = None` from contexts that don't track a current
/// sprint (e.g. the multi-card variant).
pub fn render_entry_line(
    entry: &SprintAssignEntry<'_>,
    is_checked: bool,
    is_focused: bool,
    current_sprint_id: Option<Uuid>,
    board: &Board,
) -> Line<'static> {
    match entry {
        SprintAssignEntry::Header(label) => Line::from(Span::styled(
            (*label).to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        SprintAssignEntry::None => {
            let is_current = current_sprint_id.is_none();
            let prefix = if is_checked { "[x] " } else { "[ ] " };
            let suffix = if is_current { " (current)" } else { "" };
            let style = if is_focused {
                Style::default().fg(Color::White).bg(Color::Blue)
            } else if is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(format!("{}(None){}", prefix, suffix), style))
        }
        SprintAssignEntry::ActiveOrPlanned(s) => {
            let is_current = current_sprint_id == Some(s.id);
            let prefix = if is_checked { "[x] " } else { "[ ] " };
            let suffix = if is_current { " (current)" } else { "" };
            let style = if is_focused {
                Style::default().fg(Color::White).bg(Color::Blue)
            } else if is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(
                format!("{}{}{}", prefix, s.formatted_name(board, "sprint"), suffix),
                style,
            ))
        }
        SprintAssignEntry::Completed(s) | SprintAssignEntry::Ended(s) => {
            let is_current = current_sprint_id == Some(s.id);
            let prefix = if is_checked { "[x] " } else { "[ ] " };
            let suffix = if is_current { " (current)" } else { "" };
            let status_color = if matches!(entry, SprintAssignEntry::Completed(_)) {
                Color::Green
            } else {
                Color::Red
            };
            let style = if is_focused {
                Style::default().fg(Color::White).bg(Color::Blue)
            } else {
                Style::default().fg(status_color)
            };
            Line::from(Span::styled(
                format!("{}{}{}", prefix, s.formatted_name(board, "sprint"), suffix),
                style,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use kanban_domain::{Sprint, SprintStatus};
    use kanban_view::sprint_assign_list::ACTIVE_PLANNED_HEADER;

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn make_sprint(
        sprint_number: u32,
        board_id: Uuid,
        status: SprintStatus,
        end_date: Option<DateTime<Utc>>,
    ) -> Sprint {
        Sprint {
            id: Uuid::new_v4(),
            board_id,
            sprint_number,
            name_index: None,
            prefix: None,
            card_prefix: None,
            status,
            start_date: None,
            end_date,
            created_at: ts("2026-01-01T00:00:00Z"),
            updated_at: ts("2026-01-01T00:00:00Z"),
        }
    }

    fn line_to_string(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn make_board_for_render() -> Board {
        Board::new("B", Some("TST"))
    }

    #[test]
    fn test_render_entry_line_marks_checked_with_filled_checkbox() {
        let board = make_board_for_render();
        let entry = SprintAssignEntry::None;
        let line = render_entry_line(&entry, /*is_checked=*/ true, false, None, &board);
        assert!(
            line_to_string(&line).starts_with("[x]"),
            "checked row should start with [x], got: {:?}",
            line_to_string(&line)
        );
    }

    #[test]
    fn test_render_entry_line_marks_unchecked_with_empty_checkbox() {
        let board = make_board_for_render();
        let entry = SprintAssignEntry::None;
        let line = render_entry_line(&entry, /*is_checked=*/ false, false, None, &board);
        assert!(
            line_to_string(&line).starts_with("[ ]"),
            "unchecked row should start with [ ], got: {:?}",
            line_to_string(&line)
        );
    }

    #[test]
    fn test_render_entry_line_checkbox_applies_to_sprint_rows() {
        let board = make_board_for_render();
        let sprint = make_sprint(1, board.id, SprintStatus::Planning, None);
        let entry = SprintAssignEntry::ActiveOrPlanned(&sprint);

        let checked = render_entry_line(&entry, true, false, None, &board);
        let unchecked = render_entry_line(&entry, false, false, None, &board);

        assert!(line_to_string(&checked).starts_with("[x]"));
        assert!(line_to_string(&unchecked).starts_with("[ ]"));
    }

    #[test]
    fn test_render_entry_line_header_has_no_checkbox() {
        let board = make_board_for_render();
        let entry = SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER);
        let line = render_entry_line(&entry, false, false, None, &board);
        let text = line_to_string(&line);
        assert!(
            !text.contains("[x]") && !text.contains("[ ]"),
            "section headers should not render a checkbox, got: {text:?}"
        );
    }

    #[test]
    fn test_render_entry_line_checked_and_focused_are_independent() {
        let board = make_board_for_render();
        let sprint = make_sprint(1, board.id, SprintStatus::Planning, None);
        let entry = SprintAssignEntry::ActiveOrPlanned(&sprint);

        // Checked but cursor is elsewhere: [x] without blue background.
        let checked_only = render_entry_line(&entry, true, false, None, &board);
        // Focused but not checked: [ ] with blue background.
        let focused_only = render_entry_line(&entry, false, true, None, &board);

        assert!(line_to_string(&checked_only).starts_with("[x]"));
        assert!(line_to_string(&focused_only).starts_with("[ ]"));
    }
}
