use crate::theme::{error_text, label_text};
use kanban_domain::LoadState;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn load_state_body<T>(noun: &str, state: &LoadState<&[T]>) -> Option<Line<'static>> {
    match state {
        LoadState::Loaded(_) => None,
        LoadState::NotLoaded => Some(Line::from(Span::styled(
            format!("  {noun} not loaded yet"),
            label_text(),
        ))),
        LoadState::Missing => Some(Line::from(Span::styled(
            format!("  {noun} not found"),
            label_text(),
        ))),
        LoadState::Failed(_) => Some(Line::from(Span::styled(
            format!("  {noun} failed to load"),
            error_text(),
        ))),
    }
}

pub fn render_unavailable_panel<T>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    state: &LoadState<T>,
) {
    let line = match state {
        LoadState::Loaded(_) => return,
        LoadState::NotLoaded => Line::from(Span::styled(
            format!("  {title} not loaded yet"),
            label_text(),
        )),
        LoadState::Missing => {
            Line::from(Span::styled(format!("  {title} not found"), label_text()))
        }
        LoadState::Failed(_) => Line::from(Span::styled(
            format!("  {title} failed to load"),
            error_text(),
        )),
    };
    frame.render_widget(
        Paragraph::new(vec![line]).block(
            Block::default()
                .title(title.to_string())
                .borders(Borders::ALL),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::KanbanError;
    use std::sync::Arc;

    #[test]
    fn test_load_state_body_returns_none_for_loaded() {
        let items: Vec<u32> = vec![1, 2];
        let state: LoadState<&[u32]> = LoadState::Loaded(items.as_slice());
        assert!(load_state_body("Columns", &state).is_none());
    }

    #[test]
    fn test_load_state_body_distinguishes_all_three_unavailable_variants() {
        let not_loaded: LoadState<&[u32]> = LoadState::NotLoaded;
        let missing: LoadState<&[u32]> = LoadState::Missing;
        let failed: LoadState<&[u32]> =
            LoadState::Failed(Arc::new(KanbanError::unsupported("boom")));

        let not_loaded_line = load_state_body("Columns", &not_loaded).unwrap();
        let missing_line = load_state_body("Columns", &missing).unwrap();
        let failed_line = load_state_body("Columns", &failed).unwrap();

        let render =
            |line: &Line| -> String { line.spans.iter().map(|s| s.content.as_ref()).collect() };

        assert_eq!(render(&not_loaded_line), "  Columns not loaded yet");
        assert_eq!(render(&missing_line), "  Columns not found");
        assert_eq!(render(&failed_line), "  Columns failed to load");
    }
}
