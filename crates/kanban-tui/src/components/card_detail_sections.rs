use crate::app::CommitsPanel;
use crate::components::metadata_line_multi;
use crate::theme::*;
use kanban_core::AppConfig;
use kanban_domain::{Board, Card, Sprint};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

const MAX_VISIBLE_COMMITS: usize = 4;

pub fn build_commits_lines(panel: &CommitsPanel) -> Vec<Line<'static>> {
    match panel {
        CommitsPanel::NotLoaded => {
            vec![Line::from(Span::styled("Loading commits…", label_text()))]
        }
        CommitsPanel::Unavailable => {
            vec![Line::from(Span::styled(
                "Commits unavailable",
                label_text(),
            ))]
        }
        CommitsPanel::Loaded(commits) if commits.is_empty() => {
            vec![Line::from(Span::styled("No linked commits", label_text()))]
        }
        CommitsPanel::Loaded(commits) => commits
            .iter()
            .take(MAX_VISIBLE_COMMITS)
            .map(commit_line)
            .collect(),
    }
}

fn commit_line(commit: &kanban_service::git::CommitRef) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{} ", commit.short_hash),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("· ", label_text()),
        Span::styled(commit.subject.clone(), normal_text()),
        Span::raw("  "),
        Span::styled(
            commit.committed_at.format("%Y-%m-%d").to_string(),
            label_text(),
        ),
    ])
}

pub fn build_title_lines(card: &Card) -> String {
    card.title.clone()
}

pub fn build_metadata_lines(
    card: &Card,
    board: &Board,
    sprints: &[Sprint],
    app_config: &AppConfig,
) -> Vec<Line<'static>> {
    vec![
        metadata_line_multi(vec![
            ("Priority", format!("{:?}", card.priority), normal_text()),
            ("Status", format!("{:?}", card.status), normal_text()),
            (
                "Points",
                card.points
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                normal_text(),
            ),
        ]),
        if let Some(due_date) = card.due_date {
            Line::from(Span::styled(
                format!("Due: {}", due_date.format("%Y-%m-%d %H:%M")),
                Style::default().fg(Color::Red),
            ))
        } else {
            Line::from(Span::styled("No due date", label_text()))
        },
        Line::from(Span::styled(
            format!(
                "Branch: {}",
                card.branch_name(board, sprints, app_config.effective_default_card_prefix())
            ),
            active_item(),
        )),
    ]
}

pub fn build_description_lines(card: &Card) -> Vec<Line<'static>> {
    if let Some(desc_text) = &card.description {
        if desc_text.trim().is_empty() {
            vec![Line::from(Span::styled("No description", label_text()))]
        } else {
            crate::markdown_renderer::render_markdown(desc_text)
        }
    } else {
        vec![Line::from(Span::styled("No description", label_text()))]
    }
}

pub fn build_sprint_logs_lines(card: &Card) -> Vec<Line<'static>> {
    let mut sprint_log_lines = vec![];

    let max_visible = 3;
    let total_logs = card.sprint_logs.len();
    let start_idx = total_logs.saturating_sub(max_visible);

    for (display_idx, log) in card.sprint_logs[start_idx..].iter().enumerate() {
        let absolute_idx = start_idx + display_idx;
        let sprint_name_str = log
            .sprint_name
            .as_deref()
            .unwrap_or(&log.sprint_number.to_string())
            .to_string();
        let started = log.started_at.format("%m-%d %H:%M").to_string();
        let status_str = if let Some(ended) = log.ended_at {
            let ended_fmt = ended.format("%m-%d %H:%M").to_string();
            format!("{} → {}", started, ended_fmt)
        } else {
            format!("{} → (current)", started)
        };

        sprint_log_lines.push(Line::from(vec![
            Span::styled(format!("{}. ", absolute_idx + 1), label_text()),
            Span::styled(
                format!("{} ", sprint_name_str),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(status_str, label_text()),
        ]));
    }

    if start_idx > 0 {
        sprint_log_lines.insert(
            0,
            Line::from(Span::styled(
                format!("... ({} earlier entries)", start_idx),
                label_text(),
            )),
        );
    }

    sprint_log_lines
}
