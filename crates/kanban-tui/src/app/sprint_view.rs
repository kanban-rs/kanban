use crate::card_list_component::CardListComponent;
use kanban_view::card_list::{CardList, CardListId};
use kanban_view::card_list_component::CardListComponentConfig;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SprintTaskPanel {
    Uncompleted,
    Completed,
}

pub struct SprintViewState {
    pub panel: SprintTaskPanel,
    pub uncompleted_cards: CardList,
    pub completed_cards: CardList,
    pub uncompleted_component: CardListComponent,
    pub completed_component: CardListComponent,
}

impl Default for SprintViewState {
    fn default() -> Self {
        Self {
            panel: SprintTaskPanel::Uncompleted,
            uncompleted_cards: CardList::new(CardListId::All),
            completed_cards: CardList::new(CardListId::All),
            uncompleted_component: CardListComponent::new(
                CardListId::All,
                CardListComponentConfig::default(),
            ),
            completed_component: CardListComponent::new(
                CardListId::All,
                CardListComponentConfig::default(),
            ),
        }
    }
}

impl SprintViewState {
    pub fn sync_scroll(&mut self, uncompleted_viewport: usize, completed_viewport: usize) {
        self.uncompleted_cards
            .ensure_selected_visible(uncompleted_viewport);
        self.completed_cards
            .ensure_selected_visible(completed_viewport);
    }
}
