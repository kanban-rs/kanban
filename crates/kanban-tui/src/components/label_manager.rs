//! The Manage Labels dialog.
//!
//! The dialog is an outer "Labels" block containing two bordered sub-sections:
//! the labels list (a scrollable box, internally divided by a muted divisor
//! into "on at least one card" above and "on none" below), and the **New label**
//! input box. The box holding the current cursor row is the *focused* section
//! (accent border) while the other uses a neutral border, so both the user and
//! the program can see where the cursor is. The cursor is a row-background
//! highlight inside the focused box, and the footer hint is context-aware
//! of that section.
use crate::app::{App, LabelRow};
use crate::components::centered_rect_abs;
use crate::components::radio_list::scroll_offset_to_show;
use crate::components::{set_text_field_caret, text_field_content};
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Dialog height target, as a fraction of the terminal (capped at the terminal
/// itself), plus the per-box body sizing used to split it between the two boxes.
const OUTER_PERCENT: usize = 85;
const SECTION_GAP: usize = 1;
const HINT_HEIGHT: usize = 1;
/// The labels box is the scrollable one: its body is at least this tall and it
/// absorbs the dialog's remaining height.
const LABELS_MIN_BODY: usize = 4;
/// The "New label" input body is a single text line.
const CREATE_BODY: usize = 1;
/// A partial label shows its card keys, collapsing to a count beyond this many.
const PARTIAL_KEY_LIMIT: usize = 5;

/// The two dialog sub-sections. The one holding the cursor row is the focused
/// section (accent border + bold title + context-aware hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Labels,
    Create,
}

/// Which section a flat row belongs to.
fn section_of(row: &LabelRow) -> Option<Section> {
    match row {
        LabelRow::Label(_) => Some(Section::Labels),
        LabelRow::Create => Some(Section::Create),
        LabelRow::Divider | LabelRow::Note(_) => None,
    }
}

/// Group the flat entry list into the two section bodies: everything except the
/// `Create` row belongs to the Labels box, the `Create` row to the New-label box.
fn split_body(entries: &[LabelRow]) -> (Vec<LabelRow>, Vec<LabelRow>) {
    let mut labels = Vec::new();
    let mut create = Vec::new();
    for row in entries {
        if *row == LabelRow::Create {
            create.push(*row);
        } else {
            labels.push(*row);
        }
    }
    (labels, create)
}

pub fn render_manage_labels_popup(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();

    let (labels_body, create_body) = split_body(&app.label_manager.entries());

    let cursor_row = app.label_manager.current_row();
    let focus = cursor_row
        .and_then(|r| section_of(&r))
        .unwrap_or(Section::Labels);

    let create_box_h = 2 + CREATE_BODY;
    let labels_min_box_h = 2 + LABELS_MIN_BODY;

    // Comfortable, mostly-fixed outer height: ~85% of the terminal, clamped to
    // what fits and to the minimum that keeps both boxes well-proportioned.
    let inner_min = labels_min_box_h + 2 * SECTION_GAP + create_box_h + HINT_HEIGHT;
    let outer_min = inner_min + 2;
    let frame_h = frame_area.height as usize;
    let target = (frame_h * OUTER_PERCENT) / 100;
    let outer_h = if outer_min > frame_h {
        frame_h
    } else {
        target.max(outer_min).min(frame_h)
    };

    let area = centered_rect_abs(80, outer_h as u16, frame_area);
    frame.render_widget(Clear, area);

    let outer_block = Block::default()
        .title("Labels")
        .borders(Borders::ALL)
        .border_style(focused_border())
        .style(popup_bg());
    let inner_rect = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // The labels box takes the remainder so the whole outer area is used.
    let inner_h = inner_rect.height as usize;
    let labels_box_h = inner_h
        .saturating_sub(create_box_h + 2 * SECTION_GAP + HINT_HEIGHT)
        .max(labels_min_box_h) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(labels_box_h),
            Constraint::Length(SECTION_GAP as u16),
            Constraint::Length(create_box_h as u16),
            Constraint::Length(HINT_HEIGHT as u16),
        ])
        .split(inner_rect);

    let create_insert = app.label_manager.create_insert;

    render_section(
        frame,
        chunks[0],
        "",
        focus == Section::Labels,
        false,
        &labels_body,
        app,
    );
    render_section(
        frame,
        chunks[2],
        "New label",
        focus == Section::Create,
        create_insert,
        &create_body,
        app,
    );
    if focus == Section::Create {
        set_create_input_cursor(frame, chunks[2], app);
    }
    render_hint(
        frame,
        chunks[3],
        focus,
        !app.input.as_str().is_empty(),
        create_insert,
    );
}

/// Place the terminal caret in the "New label" box so the row reads as a live
/// text field (positioned by the shared text-field component).
fn set_create_input_cursor(frame: &mut Frame, box_area: Rect, app: &App) {
    set_text_field_caret(
        frame,
        box_area,
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    )
}

/// Draw one section box: a bordered block with a title (accented when focused)
/// and a scrollable body whose current row is background-highlighted.
/// When `focused && insert_mode`, the border is red (active-typing state);
/// when `focused && !insert_mode`, the border is cyan (normal / selected).
fn render_section(
    frame: &mut Frame,
    area: Rect,
    title: &'static str,
    focused: bool,
    insert_mode: bool,
    body: &[LabelRow],
    app: &App,
) {
    if area.height < 2 {
        return;
    }
    let highlight_bg = app.highlight_bg();
    let cursor_row = app.label_manager.current_row();
    let border = if focused {
        if insert_mode {
            insert_border()
        } else {
            focused_border()
        }
    } else {
        unfocused_border()
    };
    let title_style = if focused {
        bold_highlight()
    } else {
        normal_text()
    };
    let block = Block::default()
        .title(Line::from(Span::styled(title.to_string(), title_style)))
        .borders(Borders::ALL)
        .border_style(border)
        .style(popup_bg());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let lines: Vec<Line<'static>> = body
        .iter()
        .map(|row| {
            render_row(
                *row,
                Some(*row) == cursor_row,
                highlight_bg,
                inner.width as usize,
                app,
            )
        })
        .collect();
    let selected_local = body.iter().position(|row| Some(*row) == cursor_row);
    let scroll = match selected_local {
        Some(local) => scroll_offset_to_show(local, lines.len(), inner.height as usize),
        None => 0,
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

/// Build the one-line applied-mark suffix for a label: the mark, plus the
/// trailing context (card keys for the partial state, otherwise the registry
/// description, if any).
fn applied_mark(item_name: &str, app: &App) -> (String, String) {
    let total = app.label_manager.cards.len();
    let keys = app.label_manager.applied_card_keys(item_name);
    if total > 0 && keys.len() == total {
        ("[✓]".to_string(), String::new())
    } else if keys.is_empty() {
        ("[ ]".to_string(), String::new())
    } else {
        let listing = if keys.len() > PARTIAL_KEY_LIMIT {
            format!("{} cards…", keys.len())
        } else {
            keys.join(", ")
        };
        ("[-]".to_string(), listing)
    }
}

/// Row-level rendering inside a section box.
fn render_row(
    row: LabelRow,
    is_selected: bool,
    highlight_bg: Color,
    width: usize,
    app: &App,
) -> Line<'static> {
    let style = if is_selected {
        normal_text().bg(highlight_bg)
    } else {
        normal_text()
    };
    match row {
        // The internal divisor between the applied and not-applied groups: a
        // full-width muted dash line (never selected).
        LabelRow::Divider => Line::from(Span::styled("─".repeat(width.max(1)), normal_text())),
        LabelRow::Note(text) => Line::from(Span::styled(format!("  {text}"), normal_text())),
        LabelRow::Label(i) => {
            let item = &app.label_manager.rows[i];
            let (mark, listing) = applied_mark(&item.name, app);
            // Partial state shows the cards it is on; otherwise the registry
            // description (when the label is defined) stays visible.
            let trailing = if listing.is_empty() {
                item.description.clone().unwrap_or_default()
            } else {
                listing
            };
            let mut spans: Vec<Span> = vec![
                Span::styled(format!("{mark} "), style),
                Span::styled(item.name.clone(), style),
            ];
            if !trailing.is_empty() {
                spans.push(Span::styled(format!("  {trailing}"), normal_text()));
            }
            Line::from(spans)
        }
        LabelRow::Create => {
            // The New-label field is the shared text-input component: a muted
            // placeholder while empty, the typed name otherwise (keeping the
            // row's selection background via `style`).
            text_field_content(app.input.as_str(), "Type label name...", style)
        }
    }
}

/// The context-aware footer hint text for the focused section. For the Create
/// row the hint is a live input prompt: it reads one way while the row is still
/// empty (type to create, the full editor is reachable) and a simpler way once
/// the user has typed a name (Enter just creates it), so no action repeats.
///
/// When `create_insert` is true the Esc clause says "Esc: normal mode" (ESC
/// exits insert mode and returns to normal); otherwise it says "Esc to close"
/// (ESC closes the dialog). In both modes the Esc column is stable between the
/// empty and typed states.
fn hint_text(focus: Section, create_has_text: bool, create_insert: bool) -> String {
    match focus {
        Section::Labels => {
            "j/k: navigate   Enter: apply/remove   x: delete   e: edit   Esc: close".to_string()
        }
        Section::Create => {
            let closing = if create_insert {
                "   Esc: normal mode"
            } else {
                "   Esc to close"
            };
            let front = if create_has_text {
                "Enter to create"
            } else {
                "Enter to open the full editor"
            };
            let width = "Enter to open the full editor".len();
            if create_insert {
                format!("{front:<width$}{closing}")
            } else {
                format!("{front:<width$}{closing}   i to insert")
            }
        }
    }
}

fn render_hint(
    frame: &mut Frame,
    area: Rect,
    focus: Section,
    create_has_text: bool,
    create_insert: bool,
) {
    let hint = Paragraph::new(hint_text(focus, create_has_text, create_insert)).style(
        Style::default()
            .fg(NORMAL_TEXT)
            .add_modifier(Modifier::ITALIC),
    );
    frame.render_widget(hint, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::Label;

    /// Build an App whose label manager manages the given cards (key + tags)
    /// against the given registry.
    fn state(cards: Vec<(&str, Vec<&str>)>, registry: &[&str]) -> App {
        let mut app = App::default();
        let managed: Vec<crate::app::ManagedCard> = cards
            .into_iter()
            .map(|(k, ts)| crate::app::ManagedCard {
                key: k.to_string(),
                tags: ts.into_iter().map(|s| s.to_string()).collect(),
            })
            .collect();
        app.label_manager
            .refresh(managed, registry.iter().map(|s| Label::named(*s)).collect());
        app
    }

    #[test]
    fn split_body_separates_labels_and_create() {
        let app = state(vec![("KAN-1", vec!["design"])], &["design", "p1"]);
        let (labels, create) = split_body(&app.label_manager.entries());
        // design (applied), the internal divider, then p1 (not applied).
        assert_eq!(
            labels,
            vec![LabelRow::Label(0), LabelRow::Divider, LabelRow::Label(1)]
        );
        assert_eq!(create, vec![LabelRow::Create]);
    }

    #[test]
    fn split_body_empty_labels_keep_the_note() {
        let app = state(vec![("KAN-1", vec![])], &[]);
        let (labels, create) = split_body(&app.label_manager.entries());
        assert_eq!(labels, vec![LabelRow::Note("(no labels)")]);
        assert_eq!(create, vec![LabelRow::Create]);
    }

    #[test]
    fn section_of_maps_selectable_rows() {
        assert_eq!(section_of(&LabelRow::Label(3)), Some(Section::Labels));
        assert_eq!(section_of(&LabelRow::Create), Some(Section::Create));
        assert_eq!(section_of(&LabelRow::Divider), None);
        assert_eq!(section_of(&LabelRow::Note("nope")), None);
    }

    #[test]
    fn partial_mark_lists_few_keys_and_collapses_many_to_a_count() {
        let app = state(
            vec![
                ("KAN-1", vec!["p1"]),
                ("KAN-2", vec!["p1"]),
                ("KAN-3", vec![]),
            ],
            &["p1"],
        );
        let (mark, listing) = applied_mark("p1", &app);
        assert_eq!(mark, "[-]");
        assert_eq!(listing, "KAN-1, KAN-2");

        // Six of seven cards carry p1 -> partial with more than PARTIAL_KEY_LIMIT
        // card keys, which collapses to a count.
        let app_many = state(
            vec![
                ("KAN-1", vec!["p1"]),
                ("KAN-2", vec!["p1"]),
                ("KAN-3", vec!["p1"]),
                ("KAN-4", vec!["p1"]),
                ("KAN-5", vec!["p1"]),
                ("KAN-6", vec!["p1"]),
                ("KAN-7", vec![]),
            ],
            &["p1"],
        );
        let (mark, listing) = applied_mark("p1", &app_many);
        assert_eq!(mark, "[-]");
        assert_eq!(listing, "6 cards…");
    }

    #[test]
    fn create_hint_adapts_to_input_and_mode() {
        // Normal mode (create_insert = false):
        // The closing clause reads "Esc to close" and "i to insert" is appended
        // last; both stay at fixed columns between empty and typed states.
        let empty_n = hint_text(Section::Create, false, false);
        let typed_n = hint_text(Section::Create, true, false);
        assert_eq!(
            empty_n,
            "Enter to open the full editor   Esc to close   i to insert"
        );
        assert!(
            typed_n.contains("Enter to create"),
            "typed normal: {typed_n}"
        );
        assert!(
            typed_n.ends_with("   i to insert"),
            "normal mode must end with i to insert, got: {typed_n}"
        );
        assert_eq!(
            empty_n.rfind("Esc to close"),
            typed_n.rfind("Esc to close"),
            "Esc column stable in normal mode, empty: {empty_n:?} typed: {typed_n:?}"
        );
        assert_eq!(
            empty_n.rfind("i to insert"),
            typed_n.rfind("i to insert"),
            "i to insert column stable in normal mode"
        );

        // Insert mode (create_insert = true):
        // The closing clause reads "Esc: normal mode" and also stays at a fixed
        // column between empty and typed states.
        let empty_i = hint_text(Section::Create, false, true);
        let typed_i = hint_text(Section::Create, true, true);
        assert!(
            empty_i.ends_with("Esc: normal mode"),
            "empty insert: {empty_i}"
        );
        assert!(
            typed_i.ends_with("Esc: normal mode"),
            "typed insert: {typed_i}"
        );
        assert_eq!(
            empty_i.rfind("Esc"),
            typed_i.rfind("Esc"),
            "Esc column stable in insert mode, empty: {empty_i:?} typed: {typed_i:?}"
        );

        // The Labels section hint is unaffected by input state or insert mode.
        assert_eq!(
            hint_text(Section::Labels, true, true),
            hint_text(Section::Labels, false, false)
        );
    }
}
