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

/// Resolves the App-native `FilterState`/`Model`/active-board primitives and
/// delegates the actual suffix formatting to `kanban_view::panel_titles`
/// (KAN-1059). Not `build_filter_title_suffix` — that name now belongs to
/// the moved, `&App`-free function; this is purely the call-site adapter.
pub fn filter_title_suffix(app: &App) -> Option<String> {
    kanban_view::panel_titles::build_filter_title_suffix(
        &app.filter,
        &app.model,
        app.active_board(),
    )
}

/// Resolves the App-native primitives and delegates title formatting to
/// `kanban_view::panel_titles::build_tasks_panel_title` (KAN-1059). Not
/// `build_tasks_panel_title` — that name now belongs to the moved,
/// `&App`-free function; this is purely the call-site adapter.
pub fn tasks_panel_title(app: &App, with_filter_suffix: bool) -> String {
    let active_task_list_len = app
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
    let focus_is_cards = app.focus.active == Focus::Cards;

    kanban_view::panel_titles::build_tasks_panel_title(
        active_task_list_len,
        viewing_archived_board,
        viewing_archived_cards,
        focus_is_cards,
        with_filter_suffix,
        &app.filter,
        &app.model,
        app.active_board(),
    )
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
