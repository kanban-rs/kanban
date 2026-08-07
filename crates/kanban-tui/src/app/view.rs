use crate::card_list_component::CardListComponent;
use crate::view_strategy::UnifiedViewStrategy;
use kanban_view::card_list::CardListId;
use kanban_view::card_list_component::CardListComponentConfig;
use kanban_view::view_strategy::ViewStrategy;
use ratatui::layout::Rect;

pub struct ViewState {
    pub strategy: Box<dyn ViewStrategy>,
    pub card_list_component: CardListComponent,
    pub viewport_height: usize,
    pub last_frame_area: Rect,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            strategy: Box::new(UnifiedViewStrategy::grouped()),
            card_list_component: CardListComponent::new(
                CardListId::All,
                CardListComponentConfig::new(),
            ),
            viewport_height: 20,
            last_frame_area: Rect::default(),
        }
    }
}
