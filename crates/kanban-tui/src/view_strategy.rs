use crate::render_strategy::RenderStrategy;
use kanban_view::card_list::CardList;
use kanban_view::layout_strategy::LayoutStrategy;
use kanban_view::view_strategy::{ViewRefreshContext, ViewStrategy};

pub struct UnifiedViewStrategy {
    layout_strategy: Box<dyn LayoutStrategy>,
    render_strategy: Box<dyn RenderStrategy>,
}

impl UnifiedViewStrategy {
    pub fn flat() -> Self {
        use crate::render_strategy::SinglePanelRenderer;
        use kanban_view::layout_strategy::SingleListLayout;

        Self {
            layout_strategy: Box::new(SingleListLayout::new()),
            render_strategy: Box::new(SinglePanelRenderer::flat()),
        }
    }

    pub fn grouped() -> Self {
        use crate::render_strategy::SinglePanelRenderer;
        use kanban_view::layout_strategy::VirtualUnifiedLayout;

        Self {
            layout_strategy: Box::new(VirtualUnifiedLayout::new()),
            render_strategy: Box::new(SinglePanelRenderer::grouped()),
        }
    }

    pub fn kanban() -> Self {
        use crate::render_strategy::MultiPanelRenderer;
        use kanban_view::layout_strategy::ColumnListsLayout;

        Self {
            layout_strategy: Box::new(ColumnListsLayout::new()),
            render_strategy: Box::new(MultiPanelRenderer),
        }
    }

    pub fn get_layout_strategy(&self) -> &dyn LayoutStrategy {
        self.layout_strategy.as_ref()
    }

    pub fn get_layout_strategy_mut(&mut self) -> &mut dyn LayoutStrategy {
        self.layout_strategy.as_mut()
    }

    pub fn get_render_strategy(&self) -> &dyn RenderStrategy {
        self.render_strategy.as_ref()
    }

    pub fn try_set_active_column_index(&mut self, index: usize) -> bool {
        use kanban_view::layout_strategy::ColumnListsLayout;

        if let Some(column_layout) = self
            .layout_strategy
            .as_any_mut()
            .downcast_mut::<ColumnListsLayout>()
        {
            column_layout.set_active_column_index(index);
            true
        } else {
            false
        }
    }
}

impl ViewStrategy for UnifiedViewStrategy {
    fn get_active_task_list(&self) -> Option<&CardList> {
        self.layout_strategy.get_active_task_list()
    }

    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList> {
        self.layout_strategy.get_active_task_list_mut()
    }

    fn get_all_task_lists(&self) -> Vec<&CardList> {
        self.layout_strategy.get_all_task_lists()
    }

    fn navigate_left(&mut self, select_last: bool) -> bool {
        self.layout_strategy.navigate_left(select_last)
    }

    fn navigate_right(&mut self, select_last: bool) -> bool {
        self.layout_strategy.navigate_right(select_last)
    }

    fn refresh_task_lists(&mut self, ctx: &ViewRefreshContext) {
        self.layout_strategy.refresh_lists(ctx);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn try_navigate_to_column(&mut self, index: usize) -> bool {
        self.try_set_active_column_index(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, Card, Column};
    use std::collections::HashSet;

    /// Characterization test written ahead of the KAN-1056 seam cut that
    /// splits `layout_strategy.rs`/`view_strategy.rs` across the
    /// kanban-tui/kanban-view crate boundary. Pins today's (pre-split)
    /// `UnifiedViewStrategy::flat()` behavior: cards fed in an order that
    /// does not match board sort order come back sorted by the board's
    /// default sort (card number ascending). Must keep passing unchanged
    /// after the split, proving the delegation wrapper didn't alter
    /// observable behavior.
    #[test]
    fn test_unified_view_strategy_delegates_to_kanban_view_layout_strategy() {
        let board = Board::new("Fixture", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let mut card_a = Card::new(board.id, column.id, "Card A", 0);
        card_a.card_number = 1;
        let mut card_b = Card::new(board.id, column.id, "Card B", 1);
        card_b.card_number = 2;
        let mut card_c = Card::new(board.id, column.id, "Card C", 2);
        card_c.card_number = 3;

        // Deliberately out of card-number order, to prove the query/sort
        // path actually runs rather than passing the input through as-is.
        let all_cards = vec![card_c.clone(), card_a.clone(), card_b.clone()];
        let all_columns = vec![column.clone()];

        let ctx = ViewRefreshContext {
            board: &board,
            all_cards: &all_cards,
            all_columns: &all_columns,
            all_sprints: &[],
            active_sprint_filters: HashSet::new(),
            hide_assigned_cards: false,
            search_query: None,
        };

        let mut strategy = UnifiedViewStrategy::flat();
        strategy.refresh_task_lists(&ctx);

        let list = strategy
            .get_active_task_list()
            .expect("flat layout always has one active task list");

        assert_eq!(list.cards, vec![card_a.id, card_b.id, card_c.id]);
    }
}
