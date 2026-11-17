//! Shared building blocks for a TUI dialog's text-input field.
//!
//! A *text field* is the smallest interactive input in a dialog: a value shown
//! inside a box (or as a row in a section) that reads as typed text when
//! filled and as a muted-italic *placeholder* while empty, with the terminal
//! caret over the current character. Every such field needs the same few
//! non-obvious pieces, so they live here and are shared:
//!
//! * [`placeholder_style`] — the muted-italic style for an empty field.
//! * [`text_field_content`] — the line a field shows (`value` or placeholder).
//! * [`set_text_field_caret`] — a multibyte-safe caret position inside the box.
//!
//! A dialog keeps its own border, focus handling, and footer hint, and composes
//! these into the field shape it needs. [`crate::components::label_manager`]
//! shows the pieces composed into an embedded (row) field;
//! [`crate::components::popup::render_input_popup`] shows a standing field.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

/// Muted-italic style for a text field's placeholder. Kept mid-gray (not dark
/// gray) so it stays readable on dark terminals while clearly reading as a
/// hint rather than typed content.
pub fn placeholder_style() -> Style {
    Style::default()
        .fg(Color::Rgb(160, 160, 160))
        .add_modifier(Modifier::ITALIC)
}

/// The line a text field shows: its `value` in the caller-supplied style (so
/// the caller can keep a selection background), or a muted-italic
/// `placeholder` while the value is still empty.
pub fn text_field_content(value: &str, placeholder: &str, value_style: Style) -> Line<'static> {
    if value.is_empty() {
        Line::from(Span::styled(placeholder.to_string(), placeholder_style()))
    } else {
        Line::from(Span::styled(value.to_string(), value_style))
    }
}

/// Place the terminal caret over the character at `cursor_byte_offset` inside
/// a bordered field: one column in for the left border, one row down for the
/// top border, then the *visible* columns before the cursor. Counting visible
/// columns (rather than reusing the byte offset) keeps the caret correct for
/// multibyte names, which a raw byte offset would misplace.
pub fn set_text_field_caret(frame: &mut Frame, area: Rect, value: &str, cursor_byte_offset: usize) {
    let offset = cursor_byte_offset.min(value.len());
    let before = value[..offset].chars().count();
    frame.set_cursor_position((area.x + 1 + before as u16, area.y + 1));
}
