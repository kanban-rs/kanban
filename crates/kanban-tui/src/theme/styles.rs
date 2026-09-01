use super::colors::*;
use kanban_domain::{CardPriority, SprintStatus};
use ratatui::style::{Modifier, Style};

pub fn focused_border() -> Style {
    Style::default().fg(FOCUSED_BORDER)
}

pub fn unfocused_border() -> Style {
    Style::default().fg(UNFOCUSED_BORDER)
}

pub fn deleted_view_focused_border() -> Style {
    Style::default().fg(ratatui::style::Color::Yellow)
}

pub fn selected_item(focused: bool) -> Style {
    if focused {
        Style::default().bg(SELECTED_BG)
    } else {
        Style::default()
    }
}

pub fn active_item() -> Style {
    Style::default()
        .fg(ACTIVE_ITEM)
        .add_modifier(Modifier::BOLD)
}

pub fn normal_text() -> Style {
    Style::default().fg(NORMAL_TEXT)
}

pub fn error_text() -> Style {
    Style::default().fg(ERROR_COLOR)
}

pub fn label_text() -> Style {
    Style::default().fg(LABEL_TEXT)
}

pub fn highlight_text() -> Style {
    Style::default().fg(HIGHLIGHT_TEXT)
}

pub fn bold_highlight() -> Style {
    Style::default()
        .fg(HIGHLIGHT_TEXT)
        .add_modifier(Modifier::BOLD)
}

pub fn priority_style(priority: CardPriority) -> Style {
    let color = match priority {
        CardPriority::Critical => PRIORITY_CRITICAL,
        CardPriority::High => PRIORITY_HIGH,
        CardPriority::Medium => PRIORITY_MEDIUM,
        CardPriority::Low => PRIORITY_LOW,
    };
    Style::default().fg(color)
}

pub fn points_style(points: u8) -> Style {
    let color = match points {
        1 => POINTS_1,
        2 => POINTS_2,
        3 => POINTS_3,
        4 => POINTS_4,
        5 => POINTS_5,
        _ => NORMAL_TEXT,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub fn sprint_status_style(status: SprintStatus) -> Style {
    let color = match status {
        SprintStatus::Active => STATUS_ACTIVE,
        SprintStatus::Planning => STATUS_PLANNING,
        SprintStatus::Completed => STATUS_COMPLETED,
        SprintStatus::Cancelled => STATUS_CANCELLED,
    };
    Style::default().fg(color)
}

pub fn popup_bg() -> Style {
    Style::default().bg(POPUP_BG)
}
