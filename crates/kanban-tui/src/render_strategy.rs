use crate::app::App;
use crate::components::{
    card_list_item::{render_card_list_item, CardListItemConfig},
    PanelConfig,
};
use crate::layout_strategy::ColumnBoundary;
use crate::theme::{deleted_view_focused_border, label_text};
use kanban_domain::card_lifecycle::sorted_board_columns;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub trait RenderStrategy {
    fn render(&self, app: &App, frame: &mut Frame, area: Rect);
}

fn count_headers_in_viewport(
    column_boundaries: &[ColumnBoundary],
    scroll_offset: usize,
    viewport_height: usize,
) -> usize {
    if column_boundaries.is_empty() {
        return 0;
    }

    // Count how many column headers will appear in the viewport
    // A boundary is relevant if any of its cards appear in the visible range
    let viewport_end = scroll_offset + viewport_height;

    column_boundaries
        .iter()
        .filter(|boundary| {
            let boundary_end = boundary.start_index + boundary.card_count;
            // Boundary is relevant if it overlaps with viewport range
            boundary.start_index < viewport_end && boundary_end > scroll_offset
        })
        .count()
}

pub struct SinglePanelRenderer {
    show_column_headers: bool,
}

impl SinglePanelRenderer {
    pub fn new(show_column_headers: bool) -> Self {
        Self {
            show_column_headers,
        }
    }

    pub fn flat() -> Self {
        Self::new(false)
    }

    pub fn grouped() -> Self {
        Self::new(true)
    }
}

impl RenderStrategy for SinglePanelRenderer {
    fn render(&self, app: &App, frame: &mut Frame, area: Rect) {
        let mut lines = vec![];

        if let Some(board) = app.board_in_context() {
            {
                let active_task_list = app.view.strategy.get_active_task_list();

                if self.show_column_headers {
                    if let Some(task_list) = active_task_list {
                        use crate::layout_strategy::VirtualUnifiedLayout;
                        use crate::view_strategy::UnifiedViewStrategy;

                        // Try to get column boundaries from VirtualUnifiedLayout
                        let column_boundaries = app
                            .view
                            .strategy
                            .as_any()
                            .downcast_ref::<UnifiedViewStrategy>()
                            .and_then(|unified| {
                                let layout_any = unified.get_layout_strategy().as_any();
                                layout_any.downcast_ref::<VirtualUnifiedLayout>()
                            })
                            .map(|layout| layout.get_column_boundaries().to_vec())
                            .unwrap_or_default();

                        if column_boundaries.is_empty() && task_list.is_empty() {
                            let message = if app.selection.active_board_id.is_some() {
                                "  No tasks yet. Press 'n' to create one!"
                            } else {
                                "  (Enter/Space) to add tasks"
                            };
                            lines.push(Line::from(Span::styled(message, label_text())));
                        } else {
                            let raw_viewport_height = area.height.saturating_sub(2) as usize;
                            let scroll_offset = task_list.get_scroll_offset();

                            // Calculate "above" indicator overhead (fixed based on scroll position)
                            let above_indicator_height = if scroll_offset > 0 { 1 } else { 0 };

                            // Start with space available after above indicator
                            let available_space =
                                raw_viewport_height.saturating_sub(above_indicator_height);

                            // Initial estimate: count headers based on available space
                            let initial_header_count = count_headers_in_viewport(
                                &column_boundaries,
                                scroll_offset,
                                available_space,
                            );

                            // Calculate card slots after initial header estimate
                            let initial_card_slots =
                                available_space.saturating_sub(initial_header_count);

                            // Refine: count headers for only the cards that will actually be visible
                            let refined_header_count = count_headers_in_viewport(
                                &column_boundaries,
                                scroll_offset,
                                initial_card_slots,
                            );

                            // Calculate card slots with refined header count
                            let card_slots = available_space.saturating_sub(refined_header_count);

                            // Check if we need "below" indicator based on actual visible cards
                            let below_indicator_height =
                                if scroll_offset + card_slots < task_list.len() {
                                    1
                                } else {
                                    0
                                };

                            // Final adjusted viewport: cards minus below indicator
                            let adjusted_viewport_height =
                                card_slots.saturating_sub(below_indicator_height);

                            let render_info = task_list.get_render_info(adjusted_viewport_height);

                            // Render above indicator
                            lines.extend(crate::scroll_indicators::render_above_indicator(
                                render_info.show_above_indicator,
                                render_info.cards_above_count,
                                "Task",
                            ));

                            // Render all cards with column headers interspersed
                            let mut columns_shown = std::collections::HashSet::new();
                            let sprints = app.model.sprints();

                            for card_idx in &render_info.visible_card_indices {
                                // Find which column this card belongs to
                                let card_column_idx = column_boundaries
                                    .iter()
                                    .rposition(|b| *card_idx >= b.start_index)
                                    .unwrap_or(0);

                                // Insert header for this card's column if we haven't already
                                if !columns_shown.contains(&card_column_idx) {
                                    if let Some(boundary) = column_boundaries.get(card_column_idx) {
                                        lines.push(Line::from(Span::styled(
                                            format!(
                                                "── {} ({}) ──",
                                                boundary.column_name, boundary.card_count
                                            ),
                                            ratatui::style::Style::default()
                                                .fg(ratatui::style::Color::Cyan)
                                                .add_modifier(ratatui::style::Modifier::BOLD),
                                        )));
                                        columns_shown.insert(card_column_idx);
                                    }
                                }

                                if let Some(card_id) = task_list.cards.get(*card_idx) {
                                    if let Some(card) = app.model.card_by_id(*card_id) {
                                        let is_selected =
                                            task_list.get_selected_index() == Some(*card_idx);
                                        let animation_type = app
                                            .animation
                                            .animating
                                            .get(&card.id)
                                            .map(|a| a.animation_type);
                                        let line = render_card_list_item(CardListItemConfig {
                                            card,
                                            board,
                                            sprints,
                                            is_selected,
                                            is_focused: app.focus.active
                                                == crate::app::Focus::Cards,
                                            is_multi_selected: app
                                                .multi_select
                                                .selected_cards
                                                .contains(&card.id),
                                            show_sprint_name: app
                                                .filter
                                                .active_sprint_filters
                                                .is_empty(),
                                            animation_type,
                                            search_query: app.filter.search.active_query(),
                                        });
                                        lines.push(line);
                                    }
                                }
                            }

                            // Render below indicator
                            lines.extend(crate::scroll_indicators::render_below_indicator(
                                render_info.show_below_indicator,
                                render_info.cards_below_count,
                                "Task",
                            ));
                        }
                    } else {
                        let board_columns = sorted_board_columns(board.id, app.model.columns());

                        if board_columns.is_empty() {
                            lines.push(Line::from(Span::styled(
                                "  No columns yet. Add columns in board settings.",
                                label_text(),
                            )));
                        } else {
                            let message = if app.selection.active_board_id.is_some() {
                                "  No tasks yet. Press 'n' to create one!"
                            } else {
                                "  (Enter/Space) to add tasks"
                            };
                            lines.push(Line::from(Span::styled(message, label_text())));
                        }
                    }
                } else if let Some(task_list) = app.view.strategy.get_active_task_list() {
                    if task_list.is_empty() {
                        let message = if app.selection.active_board_id.is_some() {
                            "  No tasks yet. Press 'n' to create one!"
                        } else {
                            "  (Enter/Space) to add tasks"
                        };
                        lines.push(Line::from(Span::styled(message, label_text())));
                    } else {
                        let raw_viewport_height = area.height.saturating_sub(2) as usize;

                        // Calculate indicator overhead based on actual position
                        let mut indicator_overhead = 0;
                        if task_list.get_scroll_offset() > 0 {
                            indicator_overhead += 1; // Will show "above" indicator
                        }
                        if task_list.get_scroll_offset() + raw_viewport_height < task_list.len() {
                            indicator_overhead += 1; // Will show "below" indicator
                        }

                        // Adjust viewport height to account for indicators
                        let adjusted_viewport_height =
                            raw_viewport_height.saturating_sub(indicator_overhead);

                        let render_info = task_list.get_render_info(adjusted_viewport_height);

                        lines.extend(crate::scroll_indicators::render_above_indicator(
                            render_info.show_above_indicator,
                            render_info.cards_above_count,
                            "Task",
                        ));

                        let sprints = app.model.sprints();

                        for card_idx in &render_info.visible_card_indices {
                            if let Some(card_id) = task_list.cards.get(*card_idx) {
                                if let Some(card) = app.model.card_by_id(*card_id) {
                                    let animation_type = app
                                        .animation
                                        .animating
                                        .get(&card.id)
                                        .map(|a| a.animation_type);
                                    let line = render_card_list_item(CardListItemConfig {
                                        card,
                                        board,
                                        sprints,
                                        is_selected: task_list.get_selected_index()
                                            == Some(*card_idx),
                                        is_focused: app.focus.active == crate::app::Focus::Cards,
                                        is_multi_selected: app
                                            .multi_select
                                            .selected_cards
                                            .contains(&card.id),
                                        show_sprint_name: app
                                            .filter
                                            .active_sprint_filters
                                            .is_empty(),
                                        animation_type,
                                        search_query: app.filter.search.active_query(),
                                    });
                                    lines.push(line);
                                }
                            }
                        }

                        lines.extend(crate::scroll_indicators::render_below_indicator(
                            render_info.show_below_indicator,
                            render_info.cards_below_count,
                            "Task",
                        ));
                    }
                }
            }
        } else {
            lines.push(Line::from(Span::styled(
                "  Select a project to preview tasks",
                label_text(),
            )));
        }

        let title = crate::ui::build_tasks_panel_title(app, true);

        let mut panel_config = PanelConfig::new(&title)
            .with_focus_indicator(&title)
            .focused(app.focus.active == crate::app::Focus::Cards);

        if *app.get_base_mode() == crate::app::AppMode::ArchivedCardsView
            && app.focus.active == crate::app::Focus::Cards
        {
            panel_config = panel_config.with_custom_border_style(deleted_view_focused_border());
        }

        let content = Paragraph::new(lines).block(panel_config.block());
        frame.render_widget(content, area);
    }
}

pub struct MultiPanelRenderer;

impl RenderStrategy for MultiPanelRenderer {
    fn render(&self, app: &App, frame: &mut Frame, area: Rect) {
        if let Some(board) = app.board_in_context() {
            {
                let task_lists = app.view.strategy.get_all_task_lists();

                if task_lists.is_empty() {
                    let lines = vec![Line::from(Span::styled(
                        "  No columns yet. Add columns in board settings.",
                        label_text(),
                    ))];

                    let panel_config = PanelConfig::new("Tasks")
                        .with_focus_indicator("Tasks [2]")
                        .focused(app.focus.active == crate::app::Focus::Cards);

                    let content = Paragraph::new(lines).block(panel_config.block());
                    frame.render_widget(content, area);
                    return;
                }

                let sprint_filter_suffix = crate::ui::build_filter_title_suffix(app);

                let column_count = task_lists.len();
                let column_width = 100 / column_count as u16;

                let mut constraints = vec![];
                for _ in 0..column_count {
                    constraints.push(Constraint::Percentage(column_width));
                }

                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(constraints)
                    .split(area);

                let active_task_list = app.view.strategy.get_active_task_list();
                let sprints = app.model.sprints();

                for (col_idx, task_list) in task_lists.iter().enumerate() {
                    let mut lines = vec![];

                    let card_count = task_list.len();
                    let is_focused_column = active_task_list
                        .map(|active| std::ptr::eq(*task_list, active))
                        .unwrap_or(false);

                    if task_list.is_empty() {
                        lines.push(Line::from(Span::styled("  (no tasks)", label_text())));
                    } else {
                        let raw_viewport_height = chunks[col_idx].height.saturating_sub(2) as usize;

                        // Calculate indicator overhead based on actual position
                        // This matches the logic in SinglePanelRenderer and get_adjusted_viewport_height
                        let mut indicator_overhead = 0;
                        if task_list.get_scroll_offset() > 0 {
                            indicator_overhead += 1; // Will show "above" indicator
                        }
                        if task_list.get_scroll_offset() + raw_viewport_height < task_list.len() {
                            indicator_overhead += 1; // Will show "below" indicator
                        }

                        // Adjust viewport height to account for indicators
                        let adjusted_viewport_height =
                            raw_viewport_height.saturating_sub(indicator_overhead);

                        let render_info = task_list.get_render_info(adjusted_viewport_height);

                        lines.extend(crate::scroll_indicators::render_above_indicator(
                            render_info.show_above_indicator,
                            render_info.cards_above_count,
                            "Task",
                        ));

                        for card_idx in &render_info.visible_card_indices {
                            if let Some(card_id) = task_list.cards.get(*card_idx) {
                                if let Some(card) = app.model.card_by_id(*card_id) {
                                    let is_selected = if is_focused_column {
                                        task_list.get_selected_index() == Some(*card_idx)
                                    } else {
                                        false
                                    };

                                    let animation_type = app
                                        .animation
                                        .animating
                                        .get(&card.id)
                                        .map(|a| a.animation_type);
                                    let line = render_card_list_item(CardListItemConfig {
                                        card,
                                        board,
                                        sprints,
                                        is_selected,
                                        is_focused: app.focus.active == crate::app::Focus::Cards
                                            && is_focused_column,
                                        is_multi_selected: app
                                            .multi_select
                                            .selected_cards
                                            .contains(&card.id),
                                        show_sprint_name: app
                                            .filter
                                            .active_sprint_filters
                                            .is_empty(),
                                        animation_type,
                                        search_query: app.filter.search.active_query(),
                                    });
                                    lines.push(line);
                                }
                            }
                        }

                        lines.extend(crate::scroll_indicators::render_below_indicator(
                            render_info.show_below_indicator,
                            render_info.cards_below_count,
                            "Task",
                        ));
                    }

                    let column_name =
                        if let crate::card_list::CardListId::Column(column_id) = task_list.id {
                            app.model
                                .columns()
                                .iter()
                                .find(|c| c.id == column_id)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string())
                        } else {
                            "All".to_string()
                        };

                    let mut title = if col_idx < 9 {
                        format!("{} ({}) [{}]", column_name, card_count, col_idx + 1)
                    } else {
                        format!("{} ({})", column_name, card_count)
                    };

                    if col_idx == 0 {
                        if let Some(ref suffix) = sprint_filter_suffix {
                            title.push_str(suffix);
                        }
                    }

                    let mut panel_config = PanelConfig::new(&title)
                        .with_focus_indicator(&title)
                        .focused(app.focus.active == crate::app::Focus::Cards && is_focused_column);

                    if *app.get_base_mode() == crate::app::AppMode::ArchivedCardsView
                        && is_focused_column
                    {
                        panel_config =
                            panel_config.with_custom_border_style(deleted_view_focused_border());
                    }

                    let content = Paragraph::new(lines).block(panel_config.block());
                    frame.render_widget(content, chunks[col_idx]);
                }
            }
        } else {
            let lines = vec![Line::from(Span::styled(
                "  Select a project to preview tasks",
                label_text(),
            ))];

            let panel_config = PanelConfig::new("Tasks")
                .with_focus_indicator("Tasks [2]")
                .focused(app.focus.active == crate::app::Focus::Cards);

            let content = Paragraph::new(lines).block(panel_config.block());
            frame.render_widget(content, area);
        }
    }
}
