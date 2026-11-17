//! Render for the Manage Labels dialog (`DialogMode::ManageLabels`).
//!
//! The dialog is an outer "Manage Labels" block containing three bordered
//! sub-sections: the labels **Applied** to the card, the global **Labels**
//! registry, and the **New label** action row. Because each section lives in
//! its own box, the box that holds the current cursor row (the *focused*
//! section) is accented (bright border + bold title) while the other two use
//! a neutral border, so both the user and the program can see which section
//! the cursor is in. The cursor is a row-background highlight inside the
//! focused box, and the footer hint is context-aware of that section.
use crate::app::{App, LabelRow};
use crate::components::centered_rect_abs;
use crate::components::radio_list::scroll_offset_to_show;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Dialog height target, as a fraction of the terminal (capped at the terminal
/// itself), plus the per-section body sizing used to split it between boxes.
const OUTER_PERCENT: usize = 85;
const SECTION_GAP: usize = 1;
const HINT_HEIGHT: usize = 1;
const APPLIED_MAX_BODY: usize = 8;
const REGISTRY_MIN_BODY: usize = 5;
const CREATE_BODY: usize = 1;

/// The three dialog sub-sections. The one holding the cursor row is the
/// focused section (accent border + bold title + context-aware hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Applied,
    Registry,
    Create,
}

/// Which section a flat selectable row belongs to.
fn section_of(row: &LabelRow) -> Option<Section> {
    match row {
        LabelRow::Applied(_) => Some(Section::Applied),
        LabelRow::Registry(_) => Some(Section::Registry),
        LabelRow::Create => Some(Section::Create),
        LabelRow::Header(_) | LabelRow::Note(_) => None,
    }
}

/// Group the flat entry list into the three section bodies (in applied,
/// registry, create order). Header rows become the box titles and are not part
/// of any body; `Note` rows (the "(none…)" hint for an empty section) stay in
/// their section.
fn split_body(entries: &[LabelRow]) -> (Vec<LabelRow>, Vec<LabelRow>, Vec<LabelRow>) {
    let mut applied = Vec::new();
    let mut registry = Vec::new();
    let mut create = Vec::new();
    let mut current = Section::Applied;
    for row in entries {
        match row {
            LabelRow::Header(h) => {
                current = match *h {
                    "Labels" => Section::Registry,
                    "Create" => Section::Create,
                    _ => Section::Applied,
                };
            }
            _ => match current {
                Section::Applied => applied.push(*row),
                Section::Registry => registry.push(*row),
                Section::Create => create.push(*row),
            },
        }
    }
    (applied, registry, create)
}

pub fn render_manage_labels_popup(app: &App, frame: &mut Frame) {
    let frame_area = frame.area();

    let (applied_body, registry_body, create_body) = split_body(&app.label_manager.entries());

    let cursor_row = app.label_manager.current_row();
    let focus = cursor_row
        .and_then(|r| section_of(&r))
        .unwrap_or(Section::Applied);

    // Each box is 2 border rows + its body. Applied is capped (long lists
    // scroll); the registry box absorbs the remaining height.
    let applied_box_h = 2 + applied_body.len().clamp(1, APPLIED_MAX_BODY);
    let create_box_h = 2 + CREATE_BODY;
    let registry_min_box_h = 2 + REGISTRY_MIN_BODY;

    // Comfortable, mostly-fixed outer height: ~85% of the terminal, clamped to
    // what fits and to the minimum that keeps every box well-proportioned.
    let inner_min =
        applied_box_h + 2 * SECTION_GAP + registry_min_box_h + create_box_h + HINT_HEIGHT;
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
        .title("Manage Labels")
        .borders(Borders::ALL)
        .border_style(focused_border())
        .style(popup_bg());
    let inner_rect = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // The registry box takes the remainder so the whole outer area is used.
    let inner_h = inner_rect.height as usize;
    let consumed = applied_box_h + 2 * SECTION_GAP + create_box_h + HINT_HEIGHT;
    let registry_box_h = inner_h.saturating_sub(consumed).max(registry_min_box_h) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(applied_box_h as u16),
            Constraint::Length(SECTION_GAP as u16),
            Constraint::Length(registry_box_h),
            Constraint::Length(SECTION_GAP as u16),
            Constraint::Length(create_box_h as u16),
            Constraint::Length(HINT_HEIGHT as u16),
        ])
        .split(inner_rect);

    render_section(
        frame,
        chunks[0],
        "Applied to this card",
        focus == Section::Applied,
        &applied_body,
        app,
    );
    render_section(
        frame,
        chunks[2],
        "Labels",
        focus == Section::Registry,
        &registry_body,
        app,
    );
    render_section(
        frame,
        chunks[4],
        "New label",
        focus == Section::Create,
        &create_body,
        app,
    );
    render_hint(frame, chunks[5], focus);
}

/// Draw one sub-section box: a bordered block with a title (accented when
/// focused) and a scrollable body whose current row is background-highlighted.
fn render_section(
    frame: &mut Frame,
    area: Rect,
    title: &'static str,
    focused: bool,
    body: &[LabelRow],
    app: &App,
) {
    if area.height < 2 {
        return;
    }
    let highlight_bg = app.highlight_bg();
    let cursor_row = app.label_manager.current_row();
    let border = if focused {
        focused_border()
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
        .map(|row| render_row(*row, Some(*row) == cursor_row, highlight_bg, app))
        .collect();
    let selected_local = body.iter().position(|row| Some(*row) == cursor_row);
    let scroll = match selected_local {
        Some(local) => scroll_offset_to_show(local, lines.len(), inner.height as usize),
        None => 0,
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

/// Row-level rendering inside a section box. `Header` rows become section
/// titles, so this arm is only defensive.
fn render_row(row: LabelRow, is_selected: bool, highlight_bg: Color, app: &App) -> Line<'static> {
    use std::collections::HashSet;
    let style = if is_selected {
        normal_text().bg(highlight_bg)
    } else {
        normal_text()
    };
    match row {
        LabelRow::Header(_) => Line::default(),
        LabelRow::Note(text) => Line::from(Span::styled(format!("  {text}"), normal_text())),
        LabelRow::Applied(i) => Line::from(vec![
            Span::styled("[✓] ".to_string(), style),
            Span::styled(app.label_manager.applied[i].clone(), style),
        ]),
        LabelRow::Registry(i) => {
            let label = &app.label_manager.registry[i];
            let applied: HashSet<&String> = app.label_manager.applied.iter().collect();
            let check = if applied.contains(&label.name) {
                "[✓]"
            } else {
                "[ ]"
            };
            let mut spans: Vec<Span> = vec![
                Span::styled(format!("{check} "), style),
                Span::styled(label.name.clone(), style),
            ];
            if let Some(desc) = &label.description {
                spans.push(Span::styled(format!("  {desc}"), normal_text()));
            }
            Line::from(spans)
        }
        LabelRow::Create => Line::from(Span::styled("Type label name...".to_string(), style)),
    }
}

/// The context-aware footer hint, describing the keys for the focused section.
fn render_hint(frame: &mut Frame, area: Rect, focus: Section) {
    let text = match focus {
        Section::Applied => "j/k: navigate   Enter: remove from card   Esc: close",
        Section::Registry => {
            "j/k: navigate   Enter: apply to card   x: delete   e: edit   Esc: close"
        }
        Section::Create => "j/k: navigate   Enter: create new label   Esc: close",
    };
    let hint = Paragraph::new(text).style(
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

    fn state(applied: &[&str], registry: &[&str]) -> App {
        let mut app = App::default();
        app.label_manager.applied = applied.iter().map(|s| s.to_string()).collect();
        app.label_manager.registry = registry.iter().map(|s| Label::named(*s)).collect();
        app
    }

    #[test]
    fn split_body_separates_three_sections() {
        let app = state(&["bug"], &["bug", "design"]);
        let (applied, registry, create) = split_body(&app.label_manager.entries());
        assert_eq!(applied, vec![LabelRow::Applied(0)]);
        assert_eq!(registry, vec![LabelRow::Registry(0), LabelRow::Registry(1)]);
        assert_eq!(create, vec![LabelRow::Create]);
    }

    #[test]
    fn split_body_empty_sections_keep_the_note() {
        let app = state(&[], &[]);
        let (applied, registry, create) = split_body(&app.label_manager.entries());
        assert_eq!(applied, vec![LabelRow::Note("(none)")]);
        assert_eq!(registry, vec![LabelRow::Note("(none defined yet)")]);
        assert_eq!(create, vec![LabelRow::Create]);
    }

    #[test]
    fn section_of_maps_selectable_rows() {
        assert_eq!(section_of(&LabelRow::Applied(3)), Some(Section::Applied));
        assert_eq!(section_of(&LabelRow::Registry(1)), Some(Section::Registry));
        assert_eq!(section_of(&LabelRow::Create), Some(Section::Create));
        assert_eq!(section_of(&LabelRow::Header("Labels")), None);
        assert_eq!(section_of(&LabelRow::Note("(none)")), None);
    }
}
