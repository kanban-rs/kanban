use crate::layout_strategy::LayoutStrategy;
use crate::render_strategy::RenderStrategy;
use kanban_domain::{Board, Card, Column, Sprint};
use kanban_view::card_list::CardList;
use uuid::Uuid;

pub struct ViewRefreshContext<'a> {
    pub board: &'a Board,
    pub all_cards: &'a [Card],
    pub all_columns: &'a [Column],
    pub all_sprints: &'a [Sprint],
    pub active_sprint_filters: std::collections::HashSet<Uuid>,
    pub hide_assigned_cards: bool,
    pub search_query: Option<&'a str>,
}

pub trait ViewStrategy {
    fn get_active_task_list(&self) -> Option<&CardList>;
    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList>;
    fn get_all_task_lists(&self) -> Vec<&CardList>;
    fn navigate_left(&mut self, select_last: bool) -> bool;
    fn navigate_right(&mut self, select_last: bool) -> bool;
    fn refresh_task_lists(&mut self, ctx: &ViewRefreshContext);
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn as_any(&self) -> &dyn std::any::Any;
    fn try_navigate_to_column(&mut self, _index: usize) -> bool {
        false
    }
}

pub struct UnifiedViewStrategy {
    layout_strategy: Box<dyn LayoutStrategy>,
    render_strategy: Box<dyn RenderStrategy>,
}

impl UnifiedViewStrategy {
    pub fn flat() -> Self {
        use crate::layout_strategy::SingleListLayout;
        use crate::render_strategy::SinglePanelRenderer;

        Self {
            layout_strategy: Box::new(SingleListLayout::new()),
            render_strategy: Box::new(SinglePanelRenderer::flat()),
        }
    }

    pub fn grouped() -> Self {
        use crate::layout_strategy::VirtualUnifiedLayout;
        use crate::render_strategy::SinglePanelRenderer;

        Self {
            layout_strategy: Box::new(VirtualUnifiedLayout::new()),
            render_strategy: Box::new(SinglePanelRenderer::grouped()),
        }
    }

    pub fn kanban() -> Self {
        use crate::layout_strategy::ColumnListsLayout;
        use crate::render_strategy::MultiPanelRenderer;

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
        use crate::layout_strategy::ColumnListsLayout;

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
