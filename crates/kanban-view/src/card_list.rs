use crate::list_component::ListComponent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardListId {
    All,
    Column(Uuid),
}

#[derive(Debug, Clone)]
pub struct CardListRenderInfo {
    pub visible_card_indices: Vec<usize>,
    pub show_above_indicator: bool,
    pub cards_above_count: usize,
    pub show_below_indicator: bool,
    pub cards_below_count: usize,
}

pub struct CardList {
    pub id: CardListId,
    pub cards: Vec<Uuid>,
    list: ListComponent,
}

impl CardList {
    pub fn new(id: CardListId) -> Self {
        Self {
            id,
            cards: Vec::new(),
            list: ListComponent::new(false),
        }
    }

    pub fn with_cards(id: CardListId, cards: Vec<Uuid>) -> Self {
        let mut list = Self::new(id);
        list.update_cards(cards);
        list
    }

    pub fn update_cards(&mut self, cards: Vec<Uuid>) {
        let current_card = self.get_selected_card_id();
        self.cards = cards;
        self.list.update_item_count(self.cards.len());

        if let Some(card_id) = current_card {
            if !self.select_card(card_id) {
                if !self.cards.is_empty() {
                    self.list.set_selected_index(Some(0));
                } else {
                    self.list.set_selected_index(None);
                }
            }
        } else if !self.cards.is_empty() && self.list.get_selected_index().is_some() {
            let clamped_idx = self
                .list
                .get_selected_index()
                .unwrap()
                .min(self.cards.len() - 1);
            self.list.set_selected_index(Some(clamped_idx));
        }
    }

    pub fn get_selected_card_id(&self) -> Option<Uuid> {
        self.list
            .get_selected_index()
            .and_then(|idx| self.cards.get(idx).copied())
    }

    pub fn select_card(&mut self, card_id: Uuid) -> bool {
        if let Some(idx) = self.cards.iter().position(|&id| id == card_id) {
            self.list.set_selected_index(Some(idx));
            true
        } else {
            false
        }
    }

    pub fn navigate_up(&mut self) -> bool {
        self.list.navigate_up()
    }

    pub fn navigate_down(&mut self) -> bool {
        self.list.navigate_down()
    }

    pub fn clear(&mut self) {
        self.list.set_selected_index(None);
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn get_selected_index(&self) -> Option<usize> {
        self.list.get_selected_index()
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.list.set_selected_index(index);
    }

    pub fn get_scroll_offset(&self) -> usize {
        self.list.get_scroll_offset()
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.list.set_scroll_offset(offset);
    }

    pub fn ensure_selected_visible(&mut self, viewport_height: usize) {
        self.list.ensure_selected_visible(viewport_height);
    }

    pub fn get_render_info(&self, viewport_height: usize) -> CardListRenderInfo {
        let render_info = self.list.get_render_info(viewport_height);

        CardListRenderInfo {
            visible_card_indices: render_info.visible_indices,
            show_above_indicator: render_info.show_above_indicator,
            cards_above_count: render_info.items_above,
            show_below_indicator: render_info.show_below_indicator,
            cards_below_count: render_info.items_below,
        }
    }

    pub fn jump_to_top(&mut self) {
        if !self.cards.is_empty() {
            self.list.set_selected_index(Some(0));
            self.list.set_scroll_offset(0);
        }
    }

    pub fn jump_to_bottom(&mut self, viewport_height: usize) {
        if !self.cards.is_empty() {
            let last_idx = self.cards.len() - 1;
            self.list.set_selected_index(Some(last_idx));
            self.ensure_selected_visible(viewport_height);
        }
    }

    pub fn jump_to(&mut self, index: usize) {
        self.list.jump_to(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_list_with_cards(count: usize) -> CardList {
        CardList::with_cards(
            CardListId::All,
            (0..count).map(|_| Uuid::new_v4()).collect(),
        )
    }

    #[test]
    fn test_empty_list_render_info() {
        let list = CardList::new(CardListId::All);
        let info = list.get_render_info(10);

        assert!(info.visible_card_indices.is_empty());
        assert!(!info.show_above_indicator);
        assert!(!info.show_below_indicator);
        assert_eq!(info.cards_above_count, 0);
        assert_eq!(info.cards_below_count, 0);
    }

    #[test]
    fn test_single_card_no_scrolling() {
        let list = create_list_with_cards(1);
        let info = list.get_render_info(10);

        assert_eq!(info.visible_card_indices, vec![0]);
        assert!(!info.show_above_indicator);
        assert!(!info.show_below_indicator);
        assert_eq!(info.cards_below_count, 0);
    }

    #[test]
    fn test_viewport_exactly_fits_cards() {
        let list = create_list_with_cards(5);
        let info = list.get_render_info(5);

        assert_eq!(info.visible_card_indices, vec![0, 1, 2, 3, 4]);
        assert!(!info.show_above_indicator);
        assert!(!info.show_below_indicator);
    }

    #[test]
    fn test_cards_below_indicator() {
        let list = create_list_with_cards(10);
        let info = list.get_render_info(5);

        // viewport=5 is pure card space, so 5 cards shown
        assert_eq!(info.visible_card_indices, vec![0, 1, 2, 3, 4]);
        assert!(!info.show_above_indicator);
        assert!(info.show_below_indicator);
        assert_eq!(info.cards_below_count, 5);
    }

    #[test]
    fn test_cards_above_and_below_indicators() {
        let mut list = create_list_with_cards(16);
        list.set_scroll_offset(6);

        let info = list.get_render_info(10);

        // viewport=10, starting at 6: shows cards 6-15 (10 cards)
        assert!(info.show_above_indicator);
        assert_eq!(info.cards_above_count, 6);
        assert!(!info.show_below_indicator);
        assert_eq!(info.cards_below_count, 0);
    }

    #[test]
    fn test_bug_scenario_card_15_visibility() {
        let mut list = create_list_with_cards(16);
        list.set_scroll_offset(6);

        let info = list.get_render_info(10);

        // With viewport=10 starting at 6, should see cards 6-15
        let last_visible = *info.visible_card_indices.last().unwrap();
        assert_eq!(last_visible, 15);
        assert_eq!(info.cards_below_count, 0);

        assert!(
            !info.show_below_indicator,
            "Should not show indicator since we see all remaining cards"
        );
    }

    #[test]
    fn test_scroll_to_last_card_visible() {
        let mut list = create_list_with_cards(16);
        list.set_scroll_offset(15);

        let info = list.get_render_info(10);

        // At scroll 15, only card 15 is visible
        assert_eq!(info.visible_card_indices, vec![15]);
        assert!(info.show_above_indicator);
        assert!(!info.show_below_indicator);
        assert_eq!(info.cards_below_count, 0);
    }

    #[test]
    fn test_viewport_height_accounting_for_indicators() {
        let mut list = create_list_with_cards(20);
        list.set_scroll_offset(5);

        let info = list.get_render_info(10);

        // viewport=10 is pure card space, so 10 cards shown
        assert!(info.show_above_indicator);
        assert!(info.show_below_indicator);

        let cards_shown = info.visible_card_indices.len();
        assert_eq!(cards_shown, 10);
    }

    #[test]
    fn test_navigate_down_from_middle() {
        let mut list = create_list_with_cards(20);
        list.set_selected_index(Some(5));

        list.navigate_down();

        assert_eq!(list.get_selected_index(), Some(6));
        assert_eq!(list.get_scroll_offset(), 0);
    }

    #[test]
    fn test_navigate_down_triggers_scroll() {
        let mut list = create_list_with_cards(20);
        list.set_selected_index(Some(8));
        list.set_scroll_offset(0);

        list.navigate_down();

        // With page_size=10, card 8 is visible at scroll 0
        // Moving down goes to 9, which is still visible at scroll 0
        assert_eq!(list.get_scroll_offset(), 0);
        assert_eq!(list.get_selected_index(), Some(9));
    }

    #[test]
    fn test_navigate_up_from_middle() {
        let mut list = create_list_with_cards(20);
        list.set_selected_index(Some(5));
        list.set_scroll_offset(0);

        list.navigate_up();

        assert_eq!(list.get_selected_index(), Some(4));
    }

    #[test]
    fn test_navigate_up_triggers_scroll() {
        let mut list = create_list_with_cards(20);
        list.set_selected_index(Some(5));
        list.set_scroll_offset(5);

        list.navigate_up();

        assert_eq!(list.get_scroll_offset(), 4);
    }

    #[test]
    fn test_render_info_indices_are_valid() {
        let mut list = create_list_with_cards(20);
        list.set_scroll_offset(8);

        let info = list.get_render_info(7);

        for idx in &info.visible_card_indices {
            assert!(*idx < list.cards.len(), "Index {} out of bounds", idx);
        }
    }

    #[test]
    fn test_navigate_to_last_card_with_many_cards_above() {
        let mut list = create_list_with_cards(93);
        list.set_scroll_offset(46);
        list.set_selected_index(Some(92));

        let info = list.get_render_info(48);
        assert_eq!(info.cards_above_count, 46);
        assert!(
            !info.show_below_indicator,
            "Last card should have no 'below' indicator"
        );

        list.navigate_down();

        assert_eq!(
            list.get_selected_index(),
            Some(92),
            "Should stay at last card (92)"
        );
    }

    #[test]
    fn test_select_all_cards_from_start_to_end() {
        let mut list = create_list_with_cards(93);
        list.set_selected_index(Some(0));

        for _ in 0..92 {
            let before_idx = list.get_selected_index();
            list.navigate_down();
            let after_idx = list.get_selected_index();

            if let (Some(before), Some(after)) = (before_idx, after_idx) {
                assert!(after >= before, "Selection should move down or stay");
            }
        }

        assert_eq!(
            list.get_selected_index(),
            Some(92),
            "Should reach the last card"
        );
    }

    #[test]
    fn test_indicator_space_changes_on_scroll_boundary() {
        let mut list = create_list_with_cards(93);
        list.set_scroll_offset(45);
        list.set_selected_index(Some(92));

        let info_before = list.get_render_info(50);
        assert!(
            info_before.show_above_indicator,
            "Should have 'above' indicator"
        );

        let has_below_before = info_before.show_below_indicator;

        list.navigate_down();

        let info_after = list.get_render_info(50);
        assert!(
            info_after.show_above_indicator,
            "Should still have 'above' indicator"
        );

        assert_eq!(
            list.get_selected_index(),
            Some(92),
            "Should stay at last card"
        );

        if has_below_before {
            assert!(
                !info_after.show_below_indicator,
                "Below indicator should disappear at the end"
            );
        }
    }

    #[test]
    fn test_scroll_through_all_117_cards() {
        let mut list = create_list_with_cards(117);
        list.set_selected_index(Some(0));

        let mut iterations = 0;
        while list.get_selected_index() != Some(116) && iterations < 200 {
            list.navigate_down();
            iterations += 1;
        }

        assert_eq!(
            list.get_selected_index(),
            Some(116),
            "Should reach card 116 after {} iterations",
            iterations
        );
        assert!(
            iterations < 120,
            "Should reach last card in reasonable iterations"
        );
    }
}
