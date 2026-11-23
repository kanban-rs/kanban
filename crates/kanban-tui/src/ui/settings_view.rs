use crate::app::{App, AppMode, DialogMode};
use crate::theme::{
    default_highlight_background, error_text, label_text, normal_text, to_color,
    DEFAULT_HIGHLIGHT_HEX,
};
use kanban_core::HexColor;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const BACKGROUND_HIGHLIGHT_LABEL: &str = "Background Highlight";
const HIGHLIGHT_PREVIEW_SAMPLE: &str = " Sample ";

fn background_highlight_line(
    value: String,
    selected: bool,
    highlight_bg: Color,
    preview_bg: Color,
    value_style: Style,
) -> Line<'static> {
    let label_style = if selected {
        label_text().bg(highlight_bg)
    } else {
        label_text()
    };
    let mut line = Line::from(vec![
        Span::styled(format!("{BACKGROUND_HIGHLIGHT_LABEL}: "), label_style),
        Span::styled(value, value_style),
    ]);
    line.spans.push(Span::raw(" "));
    line.spans.push(Span::styled(
        HIGHLIGHT_PREVIEW_SAMPLE,
        normal_text().bg(preview_bg),
    ));
    line
}

pub fn render_settings_view(app: &App, frame: &mut Frame, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(columns[0]);

    let config_location = kanban_service::config::effective_configuration_location(&app.app_config);

    render_settings_configuration(app, frame, left_sections[0], &config_location);
    render_settings_config_file(app, frame, left_sections[1], &config_location);
    render_settings_storage(app, frame, columns[1]);
}

fn render_settings_configuration(app: &App, frame: &mut Frame, area: Rect, _config_location: &str) {
    use crate::app::SettingsFocus;
    use crate::components::detail_view::{
        metadata_line_selectable, metadata_line_styled, FieldSectionConfig,
    };

    let config_focused = app.focus.settings_focus == SettingsFocus::Configuration;
    let config_section = FieldSectionConfig::new(" Configuration ")
        .with_focus_indicator(" Configuration [1] ")
        .focused(config_focused);
    let config_block = config_section.block();
    let config_block_inner = config_block.inner(area);
    let highlight_bg = app.highlight_bg();
    let is_editing_highlight = app.mode == AppMode::Dialog(DialogMode::EditBackgroundHighlight);
    // The Configuration panel's composition is the single source of truth in
    // crate::app::settings::ConfigRow -- the render just maps each descriptor to
    // a Line. A new row (e.g. the KAN-1806 search.cardlist.jumpto preference)
    // is added there and appears here, with the correct per-mode visibility and
    // selectability, and the selection highlight is derived from the row's id
    // rather than a hardcoded index. The two non-selectable styled rows that the
    // override + storage-configured sub-mode renders are `selectable=false`
    // descriptors; they appear here as plain styled lines and the nav bound
    // (settings_item_count) simply excludes them.
    let rows = app.config_panel_rows(crate::app::SettingsFocus::Configuration);
    let mut config_lines: Vec<Line<'_>> = Vec::with_capacity(rows.len());
    let mut selectable_count = 0;
    for row in rows.iter() {
        // `selectable_count` is the *selectable* index of the current row -- the
        // position among selectable-only rows. That's the index the nav model
        // (`settings_config.is_selected`) and the render use, because the
        // non-selectable styled rows are skipped in the count. We capture it
        // *before* incrementing so the current row's own position is used for
        // its selected-state, not the next row's.
        let list_pos = selectable_count;
        if row.selectable {
            selectable_count += 1;
        }
        if row.background_highlight_swatch {
            // The Background Highlight row is the one row whose layout is not a
            // plain "label: value" line -- it carries a preview swatch and, while
            // the user is editing it, an inline validity check. Keep that logic
            // here (it depends on the dialog input) but key its selected-state
            // on the row's *id*, not on a hardcoded index.
            let selected = config_focused && app.selection.settings_config.is_selected(list_pos);
            let line = if is_editing_highlight {
                let typed = app.input.as_str();
                let parsed = if typed.is_empty() {
                    None
                } else {
                    typed.parse::<HexColor>().ok()
                };
                let is_valid_or_empty = typed.is_empty() || parsed.is_some();
                let value_style = if is_valid_or_empty {
                    normal_text().bg(highlight_bg)
                } else {
                    error_text().bg(highlight_bg)
                };
                let preview_bg = match parsed {
                    Some(color) => to_color(color),
                    None if typed.is_empty() => default_highlight_background(),
                    None => highlight_bg,
                };
                let value = if typed.is_empty() {
                    DEFAULT_HIGHLIGHT_HEX
                } else {
                    typed
                };
                background_highlight_line(
                    format!("{value:<7}"),
                    selected,
                    highlight_bg,
                    preview_bg,
                    value_style,
                )
            } else {
                let value = app
                    .app_config
                    .background_highlight_color()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| format!("default ({DEFAULT_HIGHLIGHT_HEX})"));
                let value_style = if selected {
                    normal_text().bg(highlight_bg)
                } else {
                    normal_text()
                };
                background_highlight_line(value, selected, highlight_bg, highlight_bg, value_style)
            };
            config_lines.push(line);
            continue;
        }
        if row.selectable {
            config_lines.push(metadata_line_selectable(
                &row.label,
                &row.value,
                config_focused && app.selection.settings_config.is_selected(list_pos),
                highlight_bg,
            ));
        } else if let Some(fg) = row.non_selectable_value_color {
            config_lines.push(metadata_line_styled(
                &row.label,
                &row.value,
                Style::default().fg(fg),
            ));
        } else {
            config_lines.push(metadata_line_styled(
                &row.label,
                &row.value,
                Style::default(),
            ));
        }
    }
    let config_paragraph = Paragraph::new(config_lines).block(config_block);
    frame.render_widget(config_paragraph, area);
    if is_editing_highlight {
        let cursor_col = app.input.cursor_byte_offset() as u16;
        frame.set_cursor_position((
            config_block_inner.x + 22 + cursor_col,
            config_block_inner.y + 5,
        ));
    }
}

fn render_settings_config_file(app: &App, frame: &mut Frame, area: Rect, config_location: &str) {
    use crate::app::SettingsFocus;
    use crate::components::detail_view::{metadata_line_selectable, FieldSectionConfig};

    let config_file_focused = app.focus.settings_focus == SettingsFocus::ConfigFile;
    let config_file_section = FieldSectionConfig::new(" Config File ")
        .with_focus_indicator(" Config File [2] ")
        .focused(config_file_focused);
    let config_file_block = config_file_section.block();
    let is_cf_selected =
        |i: usize| config_file_focused && app.selection.settings_config_file.is_selected(i);
    let highlight_bg = app.highlight_bg();
    let config_path_display = if config_location.is_empty() {
        "(unknown)".to_string()
    } else {
        config_location.to_string()
    };
    let config_exists =
        !config_location.is_empty() && std::path::Path::new(config_location).exists();
    let status = if config_exists { "Loaded" } else { "Not found" };
    let config_format = app.app_config.effective_configuration_format();
    let config_file_lines = vec![
        metadata_line_selectable(
            "Path",
            &config_path_display,
            is_cf_selected(0),
            highlight_bg,
        ),
        metadata_line_selectable("Status", status, is_cf_selected(1), highlight_bg),
        metadata_line_selectable(
            "Configuration Format",
            config_format,
            is_cf_selected(2),
            highlight_bg,
        ),
    ];
    let config_file_paragraph = Paragraph::new(config_file_lines).block(config_file_block);
    frame.render_widget(config_file_paragraph, area);
}

fn render_settings_storage(app: &App, frame: &mut Frame, area: Rect) {
    use crate::app::SettingsFocus;
    use crate::components::detail_view::{metadata_line_selectable, FieldSectionConfig};

    let storage_focused = app.focus.settings_focus == SettingsFocus::Storage;
    let storage_section = FieldSectionConfig::new(" Storage ")
        .with_focus_indicator(" Storage [3] ")
        .focused(storage_focused);
    let storage_block = storage_section.block();
    let is_storage_selected =
        |i: usize| storage_focused && app.selection.settings_storage.is_selected(i);
    let highlight_bg = app.highlight_bg();
    let file_path = app.persistence.save_file.as_deref().unwrap_or("(none)");
    let backend = if app.persistence.save_file.is_some() {
        app.app_config.effective_storage_backend()
    } else {
        "(none)"
    };
    let instance_id = app.ctx.backend().instance_id().to_string();
    let export_selected = is_storage_selected(3);
    let export_checkbox_style = if export_selected {
        Style::default().fg(Color::Yellow).bg(highlight_bg)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let export_text_style = if export_selected {
        Style::default().fg(Color::White).bg(highlight_bg)
    } else {
        Style::default().fg(Color::White)
    };
    let storage_lines = vec![
        metadata_line_selectable("File", file_path, is_storage_selected(0), highlight_bg),
        metadata_line_selectable("Backend", backend, is_storage_selected(1), highlight_bg),
        metadata_line_selectable(
            "Instance ID",
            &instance_id,
            is_storage_selected(2),
            highlight_bg,
        ),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [x] ", export_checkbox_style),
            Span::styled("Export Boards", export_text_style),
        ]),
    ];
    let storage_paragraph = Paragraph::new(storage_lines).block(storage_block);
    frame.render_widget(storage_paragraph, area);
}
