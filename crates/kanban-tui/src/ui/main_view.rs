use crate::app::{App, AppMode, Focus};
use crate::components::*;
use crate::theme::*;
use crate::view_strategy::UnifiedViewStrategy;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub(super) fn render_main(app: &mut App, frame: &mut Frame, area: Rect) {
    let is_kanban_view = app.is_kanban_view();

    if is_kanban_view {
        app.view.viewport_height = area.height.saturating_sub(2) as usize;
        render_tasks(app, frame, area);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        app.view.viewport_height = chunks[1].height.saturating_sub(2) as usize;
        render_projects_panel(app, frame, chunks[0]);
        render_tasks(app, frame, chunks[1]);
    }
}

pub(super) fn render_projects_panel(app: &App, frame: &mut Frame, area: Rect) {
    let mut lines = vec![];
    // The archived-boards view shows the archived board heads in the projects
    // panel (mirroring how ArchivedCardsView shows archived cards in the tasks
    // panel); everywhere else it shows the LIVE boards. Keyed on the stack-aware
    // base mode so a confirm dialog opened over the archived view keeps the
    // archived heads + "Archived Projects" title as the underlay (matching
    // `displayed_boards()`), rather than flipping to the live set under the modal.
    let archived_view = matches!(app.get_base_mode(), AppMode::ArchivedBoardsView);
    let boards = app.displayed_boards();

    if boards.is_empty() {
        let empty = if archived_view {
            "No archived projects."
        } else {
            "No projects yet. Press 'n' to create one!"
        };
        lines.push(Line::from(Span::styled(empty, label_text())));
    } else {
        for (idx, board) in boards.iter().enumerate() {
            let config = ListItemConfig::new()
                .selected(app.selection.board.get() == Some(idx))
                .focused(app.focus.active == Focus::Boards)
                .active(app.selection.active_board_id == Some(board.id));

            lines.push(styled_list_item(&board.name, &config));
        }
    }

    let panel_config = if archived_view {
        PanelConfig::new("Archived Projects")
            .with_focus_indicator("Archived Projects [1]")
            .focused(app.focus.active == Focus::Boards)
    } else {
        PanelConfig::new("Projects")
            .with_focus_indicator("Projects [1]")
            .focused(app.focus.active == Focus::Boards)
    };

    let content = Paragraph::new(lines);
    render_panel(frame, area, &panel_config, content);
}

pub fn build_filter_title_suffix(app: &App) -> Option<String> {
    let mut filters = vec![];

    if app.filter.hide_assigned_cards {
        filters.push("Unassigned Cards".to_string());
    }

    if !app.filter.active_sprint_filters.is_empty() {
        if let Some(board) = app.active_board() {
            let mut sprint_names: Vec<String> = app
                .model
                .sprints()
                .iter()
                .filter(|s| app.filter.active_sprint_filters.contains(&s.id))
                .map(|s| s.formatted_name(board, "sprint"))
                .collect();
            sprint_names.sort();
            filters.extend(sprint_names);
        }
    }

    if filters.is_empty() {
        None
    } else {
        Some(format!(" - {}", filters.join(" + ")))
    }
}

pub fn build_tasks_panel_title(app: &App, with_filter_suffix: bool) -> String {
    let count = app
        .view
        .strategy
        .get_active_task_list()
        .map(|l| l.len())
        .unwrap_or(0);
    // Display indicator: the active board's head is archived (a pure display
    // concern — the tasks behave identically to a live board).
    let viewing_archived_board = app
        .selection
        .active_board_id
        .is_some_and(|id| app.model.archived_board_ids().contains(&id));
    // Stack-aware: key off the base mode so a confirm dialog opened OVER the
    // archived-cards view keeps the "Archive" title as the underlay, rather than
    // flipping to the live "Tasks" title while the modal is open (#428 / #414
    // finding 4). Matches `displayed_cards()`, which selects the set the same way.
    let viewing_archived_cards = *app.get_base_mode() == AppMode::ArchivedCardsView;
    let mut title = if viewing_archived_cards {
        format!("Archive [{}]", count)
    } else if viewing_archived_board {
        format!("[ARCHIVED] Tasks [2] ({})", count)
    } else if app.focus.active == Focus::Cards {
        format!("Tasks [2] ({})", count)
    } else {
        "Tasks".to_string()
    };

    if with_filter_suffix && !viewing_archived_cards {
        if let Some(suffix) = build_filter_title_suffix(app) {
            title.push_str(&suffix);
        }
    }

    title
}

pub(super) fn render_tasks(app: &App, frame: &mut Frame, area: Rect) {
    if let Some(unified_strategy) = app
        .view
        .strategy
        .as_any()
        .downcast_ref::<UnifiedViewStrategy>()
    {
        unified_strategy
            .get_render_strategy()
            .render(app, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_filter_title_suffix_no_filters_returns_none() {
        let app = App::test_default();
        assert_eq!(build_filter_title_suffix(&app), None);
    }

    #[test]
    fn test_build_filter_title_suffix_unassigned_cards_flag() {
        let mut app = App::test_default();
        app.filter.hide_assigned_cards = true;
        assert_eq!(
            build_filter_title_suffix(&app),
            Some(" - Unassigned Cards".to_string())
        );
    }

    #[test]
    fn test_build_filter_title_suffix_sprint_filter_formats_sprint_name() {
        use kanban_domain::KanbanOperations;
        let mut app = App::test_default();
        let board = app
            .ctx
            .inner_mut()
            .create_board("Test Board".to_string(), None)
            .unwrap();
        let sprint = app
            .ctx
            .inner_mut()
            .create_sprint(board.id, None, Some("Sprint".to_string()))
            .unwrap();
        let sprint_id = sprint.id;
        app.selection.active_board_id = Some(board.id);
        app.filter.active_sprint_filters.insert(sprint_id);
        app.prepare_frame();
        let suffix = build_filter_title_suffix(&app);
        assert!(
            suffix.is_some(),
            "Expected Some suffix with active sprint filter"
        );
        let suffix = suffix.unwrap();
        assert!(suffix.starts_with(" - "), "Suffix should start with ' - '");
        assert!(
            suffix.contains("Sprint"),
            "Suffix should contain sprint name"
        );
    }

    #[test]
    fn test_build_filter_title_suffix_multiple_sprint_filters_sorted_and_joined() {
        use kanban_domain::KanbanOperations;
        let mut app = App::test_default();
        let board = app
            .ctx
            .inner_mut()
            .create_board("Test Board".to_string(), None)
            .unwrap();
        let sprint_a = app
            .ctx
            .inner_mut()
            .create_sprint(board.id, None, Some("Sprint A".to_string()))
            .unwrap();
        let sprint_b = app
            .ctx
            .inner_mut()
            .create_sprint(board.id, None, Some("Sprint B".to_string()))
            .unwrap();
        app.selection.active_board_id = Some(board.id);
        app.filter.active_sprint_filters.insert(sprint_a.id);
        app.filter.active_sprint_filters.insert(sprint_b.id);
        app.prepare_frame();
        let suffix = build_filter_title_suffix(&app);
        assert_eq!(
            suffix,
            Some(" - sprint-1/Sprint A + sprint-2/Sprint B".to_string()),
            "multiple sprint filters must be sorted and joined with ' + '"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_viewing_archived_board() {
        use kanban_domain::KanbanOperations;
        let mut app = App::test_default();
        let board = app
            .ctx
            .inner_mut()
            .create_board("Test Board".to_string(), None)
            .unwrap();
        app.ctx.inner_mut().archive_board(board.id).unwrap();
        app.selection.active_board_id = Some(board.id);
        app.prepare_frame();
        assert_eq!(
            build_tasks_panel_title(&app, false),
            "[ARCHIVED] Tasks [2] (0)",
            "an archived board head should show the [ARCHIVED] prefix"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_default() {
        let app = App::test_default();
        assert_eq!(build_tasks_panel_title(&app, false), "Tasks");
    }

    #[test]
    fn test_build_tasks_panel_title_archived_view() {
        let mut app = App::test_default();
        app.mode = AppMode::ArchivedCardsView;
        assert_eq!(build_tasks_panel_title(&app, false), "Archive [0]");
    }

    #[test]
    fn test_build_tasks_panel_title_cards_focus() {
        let mut app = App::test_default();
        app.focus.active = Focus::Cards;
        assert_eq!(
            build_tasks_panel_title(&app, false),
            "Tasks [2] (0)",
            "empty board should show shortcut hint [2] and count (0)"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_with_filter_suffix() {
        let mut app = App::test_default();
        app.filter.hide_assigned_cards = true;
        let title = build_tasks_panel_title(&app, true);
        assert!(
            title.ends_with(" - Unassigned Cards"),
            "Expected title to end with filter suffix, got: {}",
            title
        );
    }

    #[test]
    fn test_build_tasks_panel_title_archived_ignores_filter_suffix() {
        let mut app = App::test_default();
        app.mode = AppMode::ArchivedCardsView;
        app.filter.hide_assigned_cards = true;
        assert_eq!(build_tasks_panel_title(&app, true), "Archive [0]");
    }
}
