use crate::app::App;
use crate::components::centered_rect;
use crate::error_log::LogLevel;
use kanban_persistence::PersistenceMetadata;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// One row in the diagnostics header. `warn` flags rows that indicate a
/// version mismatch — currently just the Writer row when the file was
/// produced by a kanban newer than this binary.
struct DiagnosticsRow {
    label: &'static str,
    value: String,
    warn: bool,
}

/// Build the header rows shown above the log entries in the Diagnostics
/// popup. Pure: takes only the inputs it renders so it can be unit-tested
/// without spinning up an `App`.
fn diagnostics_rows(
    save_file: Option<&str>,
    metadata: Option<&PersistenceMetadata>,
) -> Vec<DiagnosticsRow> {
    let mut rows: Vec<DiagnosticsRow> = Vec::new();

    rows.push(DiagnosticsRow {
        label: "File",
        value: save_file
            .map(str::to_string)
            .unwrap_or_else(|| "(in-memory)".to_string()),
        warn: false,
    });

    let format = metadata
        .and_then(|m| m.format_version)
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "unknown".to_string());
    rows.push(DiagnosticsRow {
        label: "Format",
        value: format,
        warn: false,
    });

    let writer_is_newer = metadata
        .and_then(|m| m.writer_version.as_deref())
        .map(|v| version_is_newer_than_self(v, kanban_core::KANBAN_VERSION))
        .unwrap_or(false);
    let writer = match metadata {
        Some(m) => match (m.writer_version.as_deref(), m.writer_commit.as_deref()) {
            (Some(v), Some(c)) => {
                let base = format!("kanban {v} ({})", short_commit(c));
                if writer_is_newer {
                    format!("{base} (newer than this binary)")
                } else {
                    base
                }
            }
            (Some(v), None) => {
                if writer_is_newer {
                    format!("kanban {v} (newer than this binary)")
                } else {
                    format!("kanban {v}")
                }
            }
            (None, _) => "unknown (pre-stamp file)".to_string(),
        },
        None => "(no metadata)".to_string(),
    };
    rows.push(DiagnosticsRow {
        label: "Writer",
        value: writer,
        warn: writer_is_newer,
    });

    rows.push(DiagnosticsRow {
        label: "Binary",
        value: format!(
            "kanban {} ({})",
            kanban_core::KANBAN_VERSION,
            short_commit(kanban_core::KANBAN_COMMIT)
        ),
        warn: false,
    });

    if let Some(m) = metadata {
        rows.push(DiagnosticsRow {
            label: "Saved at",
            value: m.saved_at.to_rfc3339(),
            warn: false,
        });
    }

    rows
}

fn short_commit(c: &str) -> String {
    if c == "unknown" || c.is_empty() {
        c.to_string()
    } else {
        c.chars().take(8).collect()
    }
}

/// Compare two `MAJOR.MINOR.PATCH` strings. Returns `true` when `file_version`
/// is strictly greater than `self_version`. Unparseable strings (pre-release
/// suffixes, missing dots, etc.) conservatively return `false` — the
/// diagnostics row stays calm rather than crying wolf.
fn version_is_newer_than_self(file_version: &str, self_version: &str) -> bool {
    fn parse(s: &str) -> Option<(u32, u32, u32)> {
        let mut parts = s.split('.');
        let a = parts.next()?.parse().ok()?;
        let b = parts.next()?.parse().ok()?;
        let c_raw = parts.next()?;
        // Reject anything past the patch number, including prerelease suffixes.
        if parts.next().is_some() {
            return None;
        }
        let c = c_raw.parse().ok()?;
        Some((a, b, c))
    }
    match (parse(file_version), parse(self_version)) {
        (Some(file), Some(me)) => file > me,
        _ => false,
    }
}

pub fn render_error_log_popup(app: &App, frame: &mut Frame) {
    let area = centered_rect(85, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Diagnostics [F12] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = diagnostics_rows(
        app.persistence.save_file.as_deref(),
        app.ctx.persistence_metadata().as_ref(),
    );
    let total = app.with_error_log(|log| log.entries.len());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .horizontal_margin(1)
        .constraints([
            Constraint::Length(rows.len() as u16),
            Constraint::Length(1), // blank row between diagnostics and log sections
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(inner);

    let header_lines: Vec<Line> = rows
        .into_iter()
        .map(|row| {
            let value_style = if row.warn {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            // Leading space on the label so the row content aligns with the
            // "Log entries" title below (the title carries its own leading
            // space inside the border).
            Line::from(vec![
                Span::styled(
                    format!(" {:<10}", row.label),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(row.value, value_style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(header_lines), chunks[0]);

    // Log entries sit inside a bordered block whose top border doubles as
    // the visual separator between the diagnostics rows above and the
    // entries below. The title carries the section header + entry count.
    let log_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" Log entries ({total}) "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let log_inner = log_block.inner(chunks[2]);
    frame.render_widget(log_block, chunks[2]);

    let viewport_height = log_inner.height as usize;
    let total_entries = total;

    let scroll_offset = app.ui_state.error_log_list.get_scroll_offset();
    let visible: Vec<Line> = app.with_error_log(|log| {
        log.entries
            .iter()
            .rev()
            .skip(scroll_offset)
            .take(viewport_height)
            .map(|entry| {
                let (label, color) = match entry.level {
                    LogLevel::Error => ("[ERROR]", Color::Red),
                    LogLevel::Warn => (" [WARN]", Color::Yellow),
                };
                let ts = entry.timestamp.format("%H:%M:%S").to_string();
                Line::from(vec![
                    Span::styled(
                        label,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" {} {} ", ts, entry.target)),
                    Span::raw(entry.message.clone()),
                ])
            })
            .collect()
    });

    frame.render_widget(Paragraph::new(visible), log_inner);

    let items_above = scroll_offset.min(total_entries);
    let items_below = total_entries.saturating_sub(scroll_offset + viewport_height);
    let mut footer_lines: Vec<Line> = Vec::new();
    if items_above > 0 || items_below > 0 {
        footer_lines.push(Line::from(Span::styled(
            format!("↑ {items_above} above  ↓ {items_below} below"),
            Style::default().fg(Color::DarkGray),
        )));
    }
    footer_lines.push(Line::from(Span::styled(
        "ESC/q: close | j/k: scroll | y: copy log",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));
    frame.render_widget(Paragraph::new(footer_lines), chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn rows_to_map(
        rows: Vec<DiagnosticsRow>,
    ) -> std::collections::HashMap<&'static str, (String, bool)> {
        rows.into_iter()
            .map(|r| (r.label, (r.value, r.warn)))
            .collect()
    }

    #[test]
    fn test_diagnostics_rows_with_no_save_file_and_no_metadata() {
        let rows = rows_to_map(diagnostics_rows(None, None));
        assert_eq!(rows.get("File").unwrap().0, "(in-memory)");
        assert_eq!(rows.get("Format").unwrap().0, "unknown");
        assert_eq!(rows.get("Writer").unwrap().0, "(no metadata)");
        assert!(!rows.get("Writer").unwrap().1, "no metadata must not warn");
        assert!(rows.contains_key("Binary"));
        assert!(!rows.contains_key("Saved at"));
    }

    #[test]
    fn test_diagnostics_rows_with_full_metadata() {
        let meta = PersistenceMetadata {
            instance_id: Uuid::nil(),
            saved_at: Utc.with_ymd_and_hms(2026, 5, 23, 12, 0, 0).unwrap(),
            writer_version: Some("0.6.0".into()),
            writer_commit: Some("18e98c4810dca9f69c7a894aa7a9ba009740cb1e".into()),
            format_version: Some(6),
        };
        let rows = rows_to_map(diagnostics_rows(Some("/tmp/board.json"), Some(&meta)));
        assert_eq!(rows.get("File").unwrap().0, "/tmp/board.json");
        assert_eq!(rows.get("Format").unwrap().0, "v6");
        let (writer, warn) = rows.get("Writer").unwrap();
        assert!(writer.contains("0.6.0"), "writer line: {writer}");
        assert!(
            writer.contains("18e98c48"),
            "must show short commit: {writer}"
        );
        // Writer == self_version, so no mismatch.
        assert!(!warn, "matching version must not warn");
        assert!(rows.contains_key("Saved at"));
    }

    #[test]
    fn test_diagnostics_rows_with_legacy_metadata_missing_writer_stamp() {
        let meta = PersistenceMetadata {
            instance_id: Uuid::nil(),
            saved_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            writer_version: None,
            writer_commit: None,
            format_version: Some(6),
        };
        let rows = rows_to_map(diagnostics_rows(Some("/tmp/legacy.json"), Some(&meta)));
        assert_eq!(rows.get("Writer").unwrap().0, "unknown (pre-stamp file)");
        assert!(
            !rows.get("Writer").unwrap().1,
            "missing stamp must not warn"
        );
        assert_eq!(rows.get("Format").unwrap().0, "v6");
    }

    #[test]
    fn test_diagnostics_rows_warns_when_writer_version_newer_than_binary() {
        // Construct a writer version that is guaranteed to be greater than the
        // current binary's. We bump the major component so this is robust
        // against any future CARGO_PKG_VERSION bump.
        let (major, _minor, _patch) = parse_version(kanban_core::KANBAN_VERSION).unwrap();
        let future = format!("{}.0.0", major + 1);
        let meta = PersistenceMetadata {
            instance_id: Uuid::nil(),
            saved_at: Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(),
            writer_version: Some(future.clone()),
            writer_commit: Some("abcdef0123456789".into()),
            format_version: Some(6),
        };
        let rows = rows_to_map(diagnostics_rows(Some("/tmp/future.json"), Some(&meta)));
        let (writer, warn) = rows.get("Writer").unwrap();
        assert!(*warn, "future writer must warn: {writer}");
        assert!(
            writer.contains("newer than this binary"),
            "writer line must call out the mismatch: {writer}"
        );
    }

    #[test]
    fn test_version_is_newer_handles_major_minor_patch() {
        assert!(version_is_newer_than_self("1.0.0", "0.9.9"));
        assert!(version_is_newer_than_self("0.7.0", "0.6.9"));
        assert!(version_is_newer_than_self("0.6.1", "0.6.0"));
        assert!(!version_is_newer_than_self("0.6.0", "0.6.0"));
        assert!(!version_is_newer_than_self("0.5.9", "0.6.0"));
    }

    #[test]
    fn test_version_is_newer_returns_false_on_unparseable_strings() {
        // Prerelease suffixes, leading 'v', empty strings: conservatively
        // do not warn.
        assert!(!version_is_newer_than_self("0.7.0-rc1", "0.6.0"));
        assert!(!version_is_newer_than_self("v0.7.0", "0.6.0"));
        assert!(!version_is_newer_than_self("", "0.6.0"));
        assert!(!version_is_newer_than_self("garbage", "0.6.0"));
    }

    /// Test helper exposed within the test module so the warns-when-newer
    /// test can construct a guaranteed-greater version without hard-coding it.
    fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
        let mut parts = s.split('.');
        let a = parts.next()?.parse().ok()?;
        let b = parts.next()?.parse().ok()?;
        let c = parts.next()?.parse().ok()?;
        Some((a, b, c))
    }

    #[test]
    fn test_short_commit_truncates_long_hashes() {
        assert_eq!(short_commit("18e98c4810dca9f69c7a"), "18e98c48");
    }

    #[test]
    fn test_short_commit_preserves_unknown_marker() {
        assert_eq!(short_commit("unknown"), "unknown");
    }

    #[test]
    fn test_short_commit_preserves_empty() {
        assert_eq!(short_commit(""), "");
    }
}
