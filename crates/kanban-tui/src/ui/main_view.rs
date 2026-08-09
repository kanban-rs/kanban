use crate::app::{App, AppMode, Focus};
use crate::components::*;
use crate::theme::*;
use crate::view_strategy::UnifiedViewStrategy;
use kanban_view::panel_titles::{TasksPanelKind, TasksPanelTitle};
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
        let viewport_height = area.height.saturating_sub(2) as usize;
        // Reserve room for the indicator rows themselves (mirroring the card
        // list's render path in `render_strategy.rs`), so a below indicator
        // doesn't get pushed past the panel's available rows.
        let adjusted_viewport_height = app
            .board_list
            .inner()
            .get_adjusted_viewport_height(viewport_height);
        let render_info = app.board_list.get_render_info(adjusted_viewport_height);
        let selected_idx = app.board_list.get_selected_index();

        lines.extend(crate::scroll_indicators::render_above_indicator(
            render_info.show_above_indicator,
            render_info.items_above,
            "Project",
        ));

        for idx in &render_info.visible_indices {
            if let Some(board) = boards.get(*idx) {
                let config = ListItemConfig::new()
                    .selected(selected_idx == Some(*idx))
                    .focused(app.focus.active == Focus::Boards)
                    .active(app.selection.active_board_id == Some(board.id));

                lines.push(styled_list_item(&board.name, &config));
            }
        }

        lines.extend(crate::scroll_indicators::render_below_indicator(
            render_info.show_below_indicator,
            render_info.items_below,
            "Project",
        ));
    }

    let base_title = if archived_view {
        "Archived Projects"
    } else {
        "Projects"
    };
    let search_suffix = app
        .filter
        .board_search
        .active_query()
        .filter(|q| !q.is_empty())
        .and_then(|q| format_filter_title_suffix(&[format!("\"{q}\"")]))
        .unwrap_or_default();
    let title = format!("{base_title}{search_suffix}");
    let focus_title = format!("{base_title} [1]{search_suffix}");
    let panel_config = PanelConfig::new(&title)
        .with_focus_indicator(&focus_title)
        .focused(app.focus.active == Focus::Boards);

    let content = Paragraph::new(lines);
    render_panel(frame, area, &panel_config, content);
}

/// Joins `kanban-view`'s structured filter labels into the terminal title
/// suffix (` - A + B`), or `None` when no filter is active.
pub fn format_filter_title_suffix(parts: &[String]) -> Option<String> {
    if parts.is_empty() {
        None
    } else {
        Some(format!(" - {}", parts.join(" + ")))
    }
}

/// Renders a `TasksPanelTitle` as the terminal panel title. The `[2]` hint is
/// re-inserted here because it names a TUI-only key that focuses this panel.
pub fn format_tasks_panel_title(title: &TasksPanelTitle) -> String {
    let mut rendered = match title.kind {
        TasksPanelKind::Archive => format!("Archive [{}]", title.count),
        TasksPanelKind::ArchivedBoardTasks => format!("[ARCHIVED] Tasks [2] ({})", title.count),
        TasksPanelKind::FocusedTasks => format!("Tasks [2] ({})", title.count),
        TasksPanelKind::UnfocusedTasks => "Tasks".to_string(),
    };

    if let Some(suffix) = format_filter_title_suffix(&title.filters) {
        rendered.push_str(&suffix);
    }

    rendered
}

/// Resolves the App-native `FilterState`/`Model`/active-board primitives and
/// renders the filter suffix from `kanban_view::panel_titles`' structured
/// labels.
pub fn filter_title_suffix(app: &App) -> Option<String> {
    format_filter_title_suffix(&kanban_view::panel_titles::build_filter_title_parts(
        &app.filter,
        &app.model,
        app.active_board(),
    ))
}

/// Resolves the App-native primitives, asks `kanban_view::panel_titles` for
/// the structured title, and renders it for the terminal.
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

    format_tasks_panel_title(&kanban_view::panel_titles::build_tasks_panel_title(
        active_task_list_len,
        viewing_archived_board,
        viewing_archived_cards,
        focus_is_cards,
        with_filter_suffix,
        &app.filter,
        &app.model,
        app.active_board(),
    ))
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
