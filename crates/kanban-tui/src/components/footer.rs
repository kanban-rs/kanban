use crate::app::{App, AppMode};
use crate::theme::*;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    if app.filter.search.is_active && app.mode != AppMode::Search {
        let search_text = format!("/{}", app.filter.search.query());
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
        let search_text = format!("/{}", app.filter.search.query());
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

    use crate::keybindings::KeybindingRegistry;

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
        let provider = KeybindingRegistry::get_provider(app);
        let context = provider.get_context();
        let keybindings = context
            .bindings
            .iter()
            .filter(|b| {
                !matches!(
                    b.action,
                    crate::keybindings::KeybindingAction::FocusPanel(_)
                )
            })
            .map(|b| format!("{}: {}", b.key, b.short_description))
            .collect::<Vec<_>>()
            .join(" | ");
        let component_help = component.help_text();
        format!(
            "{}{} | {}{}",
            selection_prefix, keybindings, component_help, error_badge
        )
    } else {
        let provider = KeybindingRegistry::get_provider(app);
        let context = provider.get_context();
        let keybindings = context
            .bindings
            .iter()
            .filter(|b| {
                !matches!(
                    b.action,
                    crate::keybindings::KeybindingAction::FocusPanel(_)
                )
            })
            .map(|b| format!("{}: {}", b.key, b.short_description))
            .collect::<Vec<_>>()
            .join(" | ");
        format!("{}{}{}", selection_prefix, keybindings, error_badge)
    };
    let help = Paragraph::new(help_text)
        .style(label_text())
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, area);
}
