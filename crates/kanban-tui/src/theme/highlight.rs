use super::colors::SELECTED_BG;
use kanban_core::{AppConfig, HexColor};
use ratatui::style::Color;

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

/// Canonical hex for the built-in default highlight — `SELECTED_BG`
/// (`Color::Blue`, xterm colour 4). Display/prefill only; keep in sync if
/// `SELECTED_BG` changes.
pub const DEFAULT_HIGHLIGHT_HEX: &str = "#0000FF";
