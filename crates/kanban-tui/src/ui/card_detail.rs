use crate::app::{App, CardFocus};
use crate::components::*;
use crate::theme::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Paragraph,
    Frame,
};
use uuid::Uuid;

const RELATIONSHIP_BOX_HEIGHT: u16 = 7;
const RELATIONSHIP_VIEWPORT_BORDER_HEIGHT: usize = 2;

pub(super) fn render_relationship_boxes(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    parents: &[Uuid],
    children: &[Uuid],
    child_count: usize,
) {
    let relationship_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let viewport_height =
        area.height
            .saturating_sub(RELATIONSHIP_VIEWPORT_BORDER_HEIGHT as u16) as usize;

    // Render Parents section
    let parents_config = FieldSectionConfig::new("Parents")
        .with_focus_indicator("Parents [4]")
        .focused(app.focus.card_focus == CardFocus::Parents);
    let all_cards: Vec<kanban_domain::Card> = app.model.cards().to_vec();
    let parents_lines = render_relationship_section(
        parents,
        &all_cards,
        "Parents",
        app.focus.card_focus == CardFocus::Parents,
        &app.relationship.parents_list,
        viewport_height,
    );
    let parents_widget = Paragraph::new(parents_lines).block(parents_config.block());
    frame.render_widget(parents_widget, relationship_chunks[0]);

    // Render Children section
    let children_title = format!("Children ({})", child_count);
    let children_title_focused = format!("Children ({}) [5]", child_count);
    let children_config = FieldSectionConfig::new(&children_title)
        .with_focus_indicator(&children_title_focused)
        .focused(app.focus.card_focus == CardFocus::Children);
    let children_lines = render_relationship_section(
        children,
        &all_cards,
        "Children",
        app.focus.card_focus == CardFocus::Children,
        &app.relationship.children_list,
        viewport_height,
    );
    let children_widget = Paragraph::new(children_lines).block(children_config.block());
    frame.render_widget(children_widget, relationship_chunks[1]);
}

pub(super) fn render_card_detail_view(app: &App, frame: &mut Frame, area: Rect) {
    if let Some(card) = app.get_card_for_detail_view() {
        let card = &card;
        if let Some(board) = app.active_board() {
            {
                let has_sprint_logs = !card.sprint_logs.is_empty();
                let card_id = card.id;

                // Get parent and child information
                let parents = app.model.graph().parents(card_id);
                let children = app.model.graph().children(card_id);
                let child_count = children.len();

                let constraints = vec![
                    Constraint::Length(5),                       // Title
                    Constraint::Length(6),                       // Metadata
                    Constraint::Min(5),                          // Description
                    Constraint::Length(RELATIONSHIP_BOX_HEIGHT), // Relationships
                ];

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(constraints)
                    .split(area);

                // Render title section
                let title_config = FieldSectionConfig::new("Task Title")
                    .with_focus_indicator("Task Title [1]")
                    .focused(app.focus.card_focus == CardFocus::Title);
                let title = Paragraph::new(build_title_lines(card))
                    .style(bold_highlight())
                    .block(title_config.block());
                frame.render_widget(title, chunks[0]);

                if has_sprint_logs {
                    let meta_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(chunks[1]);

                    // Render metadata
                    let meta_config = FieldSectionConfig::new("Metadata")
                        .with_focus_indicator("Metadata [2]")
                        .focused(app.focus.card_focus == CardFocus::Metadata);
                    let meta_lines =
                        build_metadata_lines(card, board, app.model.sprints(), &app.app_config);
                    let meta = Paragraph::new(meta_lines).block(meta_config.block());
                    frame.render_widget(meta, meta_chunks[0]);

                    // Render sprint logs
                    let sprint_logs_config = FieldSectionConfig::new("Sprint History");
                    let sprint_log_lines = build_sprint_logs_lines(card);
                    let sprint_logs =
                        Paragraph::new(sprint_log_lines).block(sprint_logs_config.block());
                    frame.render_widget(sprint_logs, meta_chunks[1]);

                    // Render description
                    let desc_config = FieldSectionConfig::new("Description")
                        .with_focus_indicator("Description [3]")
                        .focused(app.focus.card_focus == CardFocus::Description);
                    let desc_lines = build_description_lines(card);
                    let desc = Paragraph::new(desc_lines).block(desc_config.block());
                    frame.render_widget(desc, chunks[2]);

                    // Render relationship boxes
                    render_relationship_boxes(
                        app,
                        frame,
                        chunks[3],
                        &parents,
                        &children,
                        child_count,
                    );
                } else {
                    // Render metadata section
                    let meta_config = FieldSectionConfig::new("Metadata")
                        .with_focus_indicator("Metadata [2]")
                        .focused(app.focus.card_focus == CardFocus::Metadata);
                    let meta_lines =
                        build_metadata_lines(card, board, app.model.sprints(), &app.app_config);
                    let meta = Paragraph::new(meta_lines).block(meta_config.block());
                    frame.render_widget(meta, chunks[1]);

                    // Render description section
                    let desc_config = FieldSectionConfig::new("Description")
                        .with_focus_indicator("Description [3]")
                        .focused(app.focus.card_focus == CardFocus::Description);
                    let desc_lines = build_description_lines(card);
                    let desc = Paragraph::new(desc_lines).block(desc_config.block());
                    frame.render_widget(desc, chunks[2]);

                    // Render relationship boxes
                    render_relationship_boxes(
                        app,
                        frame,
                        chunks[3],
                        &parents,
                        &children,
                        child_count,
                    );
                }
            }
        }
    }
}
