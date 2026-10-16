use super::colors::SELECTED_BG;
use kanban_core::{AppConfig, HexColor};
use ratatui::style::Color;

/// Re-export: canonical hex of the built-in default highlight
/// (single source of truth in kanban-service).
pub use kanban_service::config::DEFAULT_HIGHLIGHT_HEX;

pub fn to_color(color: HexColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

pub fn highlight_background(config: &AppConfig) -> Color {
    config
        .background_highlight_color()
        .map(to_color)
        .unwrap_or(SELECTED_BG)
}

pub fn default_highlight_background() -> Color {
    SELECTED_BG
}
