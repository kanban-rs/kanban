use crate::app::{App, AppMode};
use crate::theme::*;
use kanban_view::search::SearchState;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Whichever `SearchState` is actually active — `board_search` (the projects
/// panel), `column_search` (the board detail column list), or `search` (the
/// tasks panel) — mirroring the routing `handle_search_mode`
/// (`app/input_router.rs`) already uses to decide which field typed
/// keystrokes go to. At most one of the three is active at a time.
fn active_search(app: &App) -> Option<&SearchState> {
    if app.filter.board_search.is_active {
        Some(&app.filter.board_search)
    } else if app.filter.column_search.is_active {
        Some(&app.filter.column_search)
    } else if app.filter.search.is_active {
        Some(&app.filter.search)
    } else {
        None
    }
}

pub fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    if let Some(search) = active_search(app).filter(|_| app.mode != AppMode::Search) {
        let search_text = format!("/{}", search.query());
        let help_text = "j/k: navigate | ESC: clear";

        let available_width = area.width.saturating_sub(4);
        let help_len = help_text.len() as u16;
        let search_len = search_text.len() as u16;

        let padding = if available_width > search_len + help_len + 1 {
            available_width
                .saturating_sub(search_len)
                .saturating_sub(help_len)
        } else {
            1
        };

        let footer_line = Line::from(vec![
            Span::styled(search_text, Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:width$}", "", width = padding as usize),
                label_text(),
            ),
            Span::styled(help_text, label_text()),
        ]);

        let help = Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, area);
        return;
    }

    if app.mode == AppMode::Search {
        let query = active_search(app).map(SearchState::query).unwrap_or("");
        let search_text = format!("/{query}");
        let help_text = "ESC: clear | Enter: apply";

        let available_width = area.width.saturating_sub(4);
        let help_len = help_text.len() as u16;
        let search_len = search_text.len() as u16;

        let padding = if available_width > search_len + help_len + 1 {
            available_width
                .saturating_sub(search_len)
                .saturating_sub(help_len)
        } else {
            1
        };

        let footer_line = Line::from(vec![
            Span::styled(search_text, Style::default().fg(Color::White)),
            Span::styled(
                format!("{:width$}", "", width = padding as usize),
                label_text(),
            ),
            Span::styled(help_text, label_text()),
        ]);

        let help = Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, area);
        return;
    }

    let selection_prefix = if app.multi_select.selection_mode_active {
        format!(
            "-- SELECT ({}) -- | ",
            app.multi_select.selected_cards.len()
        )
    } else if !app.multi_select.selected_cards.is_empty() {
        format!("({} selected) | ", app.multi_select.selected_cards.len())
    } else {
        String::new()
    };

    let error_badge: String = {
        let (unread_count,) = app.with_error_log(|log| (log.unread_count,));
        if unread_count > 0 {
            format!("  [!] {} new  F12: diagnostics", unread_count)
        } else {
            String::new()
        }
    };

    let help_text: String = if let AppMode::SprintDetail = app.mode {
        let component = match app.sprint_view.panel {
            crate::app::SprintTaskPanel::Uncompleted => &app.sprint_view.uncompleted_component,
            crate::app::SprintTaskPanel::Completed => &app.sprint_view.completed_component,
        };
        let keybindings = footer_keybindings_text(app);
        let component_help = component.help_text();
        format!(
            "{}{} | {}{}",
            selection_prefix, keybindings, component_help, error_badge
        )
    } else {
        let keybindings = footer_keybindings_text(app);
        format!("{}{}{}", selection_prefix, keybindings, error_badge)
    };
    let help = Paragraph::new(help_text)
        .style(label_text())
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, area);
}

/// The footer's shortcut list for the active context, with `panel N` focus
/// hints filtered out (issue #361): panel numbers already show as `[1]`/`[2]`
/// in the panel titles, and the keys still work and appear in the `?` help.
fn footer_keybindings_text(app: &App) -> String {
    use crate::keybindings::{KeybindingAction, KeybindingRegistry};

    let provider = KeybindingRegistry::get_provider(app);
    provider
        .get_context()
        .bindings
        .iter()
        .filter(|b| !matches!(b.action, KeybindingAction::FocusPanel(_)))
        .map(|b| format!("{}: {}", b.key, b.short_description))
        .collect::<Vec<_>>()
        .join(" | ")
}
