use crate::app::{App, AppMode, DialogMode};
use crate::theme::{default_highlight_background, error_text, label_text, normal_text, to_color};
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

fn render_settings_configuration(app: &App, frame: &mut Frame, area: Rect, config_location: &str) {
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
    let is_config_selected =
        |i: usize| config_focused && app.selection.settings_config.is_selected(i);
    let highlight_bg = app.highlight_bg();
    let is_editing_highlight = app.mode == AppMode::Dialog(DialogMode::EditBackgroundHighlight);
    let highlight_row_selected =
        is_config_selected(crate::handlers::settings_handlers::BACKGROUND_HIGHLIGHT_CONFIG_INDEX);
    let background_highlight_row = if is_editing_highlight {
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
        background_highlight_line(
            format!("{typed:<7}"),
            highlight_row_selected,
            highlight_bg,
            preview_bg,
            value_style,
        )
    } else {
        let value = app
            .app_config
            .background_highlight_color()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "default".to_string());
        let value_style = if highlight_row_selected {
            normal_text().bg(highlight_bg)
        } else {
            normal_text()
        };
        background_highlight_line(
            value,
            highlight_row_selected,
            highlight_bg,
            highlight_bg,
            value_style,
        )
    };
    let mut config_lines = vec![
        metadata_line_selectable(
            "Configuration Format",
            app.app_config.effective_configuration_format(),
            is_config_selected(0),
            highlight_bg,
        ),
        metadata_line_selectable(
            "Configuration Location",
            config_location,
            is_config_selected(1),
            highlight_bg,
        ),
        metadata_line_selectable(
            "Default Card Prefix",
            app.app_config.effective_default_card_prefix(),
            is_config_selected(2),
            highlight_bg,
        ),
        metadata_line_selectable(
            "Default Sprint Prefix",
            app.app_config.effective_default_sprint_prefix(),
            is_config_selected(3),
            highlight_bg,
        ),
        metadata_line_selectable(
            "Editing Format",
            app.app_config.effective_editing_format(),
            is_config_selected(4),
            highlight_bg,
        ),
        background_highlight_row,
    ];
    if app.has_data_file {
        let active_storage_location =
            kanban_service::config::resolve_storage_location(&app.app_config);
        if app.cli_file_override {
            let storage_configured =
                app.original_storage_backend.is_some() || app.original_storage_location.is_some();
            if storage_configured {
                config_lines.push(metadata_line_styled(
                    "Storage Backend",
                    &app.config_storage_backend,
                    Style::default().fg(Color::DarkGray),
                ));
                config_lines.push(metadata_line_styled(
                    "Storage Location",
                    &app.config_storage_location,
                    Style::default().fg(Color::DarkGray),
                ));
            }
            config_lines.push(metadata_line_selectable(
                "Active Storage Backend",
                app.app_config.effective_storage_backend(),
                is_config_selected(8),
                highlight_bg,
            ));
            config_lines.push(metadata_line_selectable(
                "Active Storage Location",
                &active_storage_location,
                is_config_selected(9),
                highlight_bg,
            ));
        } else if app.cli_file_provided {
            config_lines.push(metadata_line_selectable(
                "Active Storage Backend",
                app.app_config.effective_storage_backend(),
                is_config_selected(6),
                highlight_bg,
            ));
            config_lines.push(metadata_line_selectable(
                "Active Storage Location",
                &active_storage_location,
                is_config_selected(7),
                highlight_bg,
            ));
        } else {
            config_lines.push(metadata_line_selectable(
                "Storage Backend",
                app.app_config.effective_storage_backend(),
                is_config_selected(6),
                highlight_bg,
            ));
            config_lines.push(metadata_line_selectable(
                "Storage Location",
                &active_storage_location,
                is_config_selected(7),
                highlight_bg,
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
