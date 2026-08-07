use ratatui::{layout::Rect, text::Line, widgets::Paragraph, Frame};

pub struct ListItem<T> {
    pub value: T,
    pub label: Line<'static>,
    pub selectable: bool,
}

impl<T> ListItem<T> {
    pub fn selectable(value: T, label: Line<'static>) -> Self {
        Self {
            value,
            label,
            selectable: true,
        }
    }

    pub fn header(value: T, label: Line<'static>) -> Self {
        Self {
            value,
            label,
            selectable: false,
        }
    }
}

type StickyHeaderFn<'a, T> =
    Box<dyn Fn(&[ListItem<T>], usize) -> Option<(usize, Line<'static>)> + 'a>;

pub struct RadioList<'a, T> {
    items: &'a [ListItem<T>],
    sticky_header_for: Option<StickyHeaderFn<'a, T>>,
}

impl<'a, T> RadioList<'a, T> {
    pub fn new(items: &'a [ListItem<T>]) -> Self {
        Self {
            items,
            sticky_header_for: None,
        }
    }

    pub fn with_sticky_header<F>(mut self, f: F) -> Self
    where
        F: Fn(&[ListItem<T>], usize) -> Option<(usize, Line<'static>)> + 'a,
    {
        self.sticky_header_for = Some(Box::new(f));
        self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, selected: Option<usize>) {
        let lines: Vec<Line<'static>> = self.items.iter().map(|i| i.label.clone()).collect();
        let total = lines.len();
        let height = area.height as usize;
        let sel = selected.unwrap_or(0);
        let scroll = scroll_offset_to_show(sel, total, height);
        let list = Paragraph::new(lines).scroll((scroll as u16, 0));
        frame.render_widget(list, area);

        if let Some(f) = &self.sticky_header_for {
            if area.height == 0 {
                return;
            }
            if let Some((header_idx, label)) = f(self.items, sel) {
                if header_idx < scroll {
                    let overlay = Paragraph::new(label);
                    let top_row = Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    };
                    frame.render_widget(overlay, top_row);
                }
            }
        }
    }

    pub fn next_selectable(&self, cur: Option<usize>) -> Option<usize> {
        kanban_view::list_nav::next_selectable_index(cur, self.items.len(), |i| {
            self.items[i].selectable
        })
    }

    pub fn prev_selectable(&self, cur: Option<usize>) -> Option<usize> {
        kanban_view::list_nav::prev_selectable_index(cur, self.items.len(), |i| {
            self.items[i].selectable
        })
    }

    pub fn first_selectable(&self) -> Option<usize> {
        kanban_view::list_nav::first_selectable_index(self.items.len(), |i| {
            self.items[i].selectable
        })
    }
}

pub fn scroll_offset_to_show(selected: usize, total: usize, height: usize) -> usize {
    if height == 0 || total <= height || selected < height {
        return 0;
    }
    let max_offset = total.saturating_sub(height);
    (selected + 1).saturating_sub(height).min(max_offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::Span;
    use ratatui::Terminal;

    fn item(value: &'static str, label: &'static str) -> ListItem<&'static str> {
        ListItem::selectable(value, Line::from(label.to_string()))
    }

    fn header_item(label: &'static str) -> ListItem<&'static str> {
        ListItem::header("", Line::from(label.to_string()))
    }

    fn render_to_buffer(
        list: &RadioList<&'static str>,
        selected: Option<usize>,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, width, height);
                list.render(frame, area, selected);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn test_radio_list_render_emits_one_line_per_item() {
        let items = vec![item("a", "alpha"), item("b", "bravo"), item("c", "charlie")];
        let list = RadioList::new(&items);
        let out = render_to_buffer(&list, Some(0), 20, 5);
        assert!(out.contains("alpha"), "row 0 label must appear: {}", out);
        assert!(out.contains("bravo"), "row 1 label must appear: {}", out);
        assert!(out.contains("charlie"), "row 2 label must appear: {}", out);
    }

    #[test]
    fn test_radio_list_render_marks_selected_item_with_prefix() {
        let make_styled = |label: &'static str, is_selected: bool| {
            let prefix = if is_selected { "> " } else { "  " };
            ListItem::selectable(label, Line::from(format!("{}{}", prefix, label)))
        };
        let items = vec![
            make_styled("alpha", false),
            make_styled("bravo", true),
            make_styled("charlie", false),
        ];
        let list = RadioList::new(&items);
        let out = render_to_buffer(&list, Some(1), 20, 5);
        assert!(
            out.contains("> bravo"),
            "selected row should have arrow prefix: {}",
            out
        );
        assert!(
            out.contains("  alpha"),
            "non-selected row should have space prefix: {}",
            out
        );
    }

    #[test]
    fn test_radio_list_next_selectable_skips_non_selectable_items() {
        let items = vec![
            item("a", "alpha"),
            header_item("--- HDR ---"),
            item("b", "bravo"),
            header_item("--- HDR2 ---"),
            item("c", "charlie"),
        ];
        let list = RadioList::new(&items);
        assert_eq!(list.next_selectable(Some(0)), Some(2));
        assert_eq!(list.next_selectable(Some(2)), Some(4));
    }

    #[test]
    fn test_radio_list_prev_selectable_skips_non_selectable_items() {
        let items = vec![
            item("a", "alpha"),
            header_item("--- HDR ---"),
            item("b", "bravo"),
            header_item("--- HDR2 ---"),
            item("c", "charlie"),
        ];
        let list = RadioList::new(&items);
        assert_eq!(list.prev_selectable(Some(4)), Some(2));
        assert_eq!(list.prev_selectable(Some(2)), Some(0));
    }

    #[test]
    fn test_radio_list_first_selectable_returns_first_selectable_index() {
        let items = vec![
            header_item("--- HDR ---"),
            item("a", "alpha"),
            item("b", "bravo"),
        ];
        let list = RadioList::new(&items);
        assert_eq!(list.first_selectable(), Some(1));
    }

    #[test]
    fn test_radio_list_with_sticky_header_overlay_renders_label_when_selection_scrolls_past_header()
    {
        let mut items = vec![item("0", "row0")];
        items.push(header_item("STICKY"));
        for i in 1..=10 {
            items.push(item("x", Box::leak(format!("row{}", i).into_boxed_str())));
        }
        let list = RadioList::new(&items).with_sticky_header(|entries, idx| {
            for i in (0..idx).rev() {
                if !entries[i].selectable {
                    let label_text = entries[i]
                        .label
                        .spans
                        .first()
                        .map(|s| s.content.to_string())
                        .unwrap_or_default();
                    return Some((
                        i,
                        Line::from(Span::styled(
                            label_text,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )),
                    ));
                }
            }
            None
        });
        let out = render_to_buffer(&list, Some(8), 20, 5);
        assert!(
            out.contains("STICKY"),
            "sticky header overlay must show when selection scrolls past it: {}",
            out
        );
    }

    #[test]
    fn test_scroll_offset_is_zero_when_list_fits_in_viewport() {
        assert_eq!(scroll_offset_to_show(0, 3, 5), 0);
        assert_eq!(scroll_offset_to_show(2, 3, 5), 0);
        assert_eq!(scroll_offset_to_show(5, 6, 6), 0);
    }

    #[test]
    fn test_scroll_offset_is_zero_when_selected_is_in_first_viewport() {
        assert_eq!(scroll_offset_to_show(0, 20, 5), 0);
        assert_eq!(scroll_offset_to_show(4, 20, 5), 0);
    }

    #[test]
    fn test_scroll_offset_keeps_selected_visible_when_below_viewport() {
        assert_eq!(scroll_offset_to_show(5, 20, 5), 1);
        assert_eq!(scroll_offset_to_show(10, 20, 5), 6);
    }

    #[test]
    fn test_scroll_offset_clamps_at_last_full_viewport() {
        assert_eq!(scroll_offset_to_show(19, 20, 5), 15);
    }

    #[test]
    fn test_scroll_offset_handles_zero_height() {
        assert_eq!(scroll_offset_to_show(5, 20, 0), 0);
    }

    #[test]
    fn test_scroll_offset_handles_empty_list() {
        assert_eq!(scroll_offset_to_show(0, 0, 5), 0);
    }
}
