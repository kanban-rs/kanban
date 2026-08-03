use crate::components::ListItemConfig;
use crate::theme::*;
use kanban_domain::Card;
use kanban_view::ListComponent;
use ratatui::text::{Line, Span};
use uuid::Uuid;

pub fn render_relationship_section(
    card_ids: &[Uuid],
    all_cards: &[Card],
    title: &str,
    is_focused: bool,
    list_component: &ListComponent,
    viewport_height: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    if card_ids.is_empty() {
        // Empty case
        let empty_text = match title {
            "Parents" => "No parents",
            "Children" => "No children",
            _ => "No items",
        };
        lines.push(Line::from(Span::styled(empty_text, label_text())));
    } else {
        // Get page info for scrolling
        let page_info = list_component.get_render_info(viewport_height);

        // Above indicator
        lines.extend(crate::scroll_indicators::render_above_indicator(
            page_info.show_above_indicator,
            page_info.items_above,
            "item",
        ));

        // Render visible items
        for &idx in &page_info.visible_indices {
            if let Some(&card_id) = card_ids.get(idx) {
                if let Some(card) = all_cards.iter().find(|c| c.id == card_id) {
                    let is_selected = list_component.selection.get() == Some(idx);

                    let config = ListItemConfig::new()
                        .selected(is_selected)
                        .focused(is_selected && is_focused);

                    let style = config.item_style();

                    let line = Line::from(vec![
                        Span::styled("→ ", label_text()),
                        Span::styled(card.title.clone(), style),
                    ]);
                    lines.push(line);
                }
            }
        }

        // Below indicator
        lines.extend(crate::scroll_indicators::render_below_indicator(
            page_info.show_below_indicator,
            page_info.items_below,
            "item",
        ));
    }

    // Pad to viewport_height with blank lines
    while lines.len() < viewport_height {
        lines.push(Line::from(""));
    }

    lines
}
