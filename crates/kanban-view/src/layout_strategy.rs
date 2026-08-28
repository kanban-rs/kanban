use crate::card_list::{CardList, CardListId};
use crate::view_strategy::ViewRefreshContext;
use kanban_domain::card_lifecycle::sorted_board_columns;
use kanban_domain::CardQueryBuilder;
use uuid::Uuid;

fn build_query<'a>(ctx: &'a ViewRefreshContext<'a>) -> CardQueryBuilder<'a> {
    let mut builder =
        CardQueryBuilder::new(ctx.all_cards, ctx.all_columns, ctx.all_sprints, ctx.board);
    if !ctx.active_sprint_filters.is_empty() {
        builder = builder.in_sprints(ctx.active_sprint_filters.iter().copied());
    }
    if ctx.hide_assigned_cards {
        builder = builder.hide_assigned();
    }
    if let Some(query) = ctx.search_query {
        builder = builder.search(query);
    }
    builder
}

#[derive(Clone)]
pub struct ColumnBoundary {
    pub column_id: Uuid,
    pub column_name: String,
    pub start_index: usize,
    pub card_count: usize,
}

pub trait LayoutStrategy {
    fn get_active_task_list(&self) -> Option<&CardList>;
    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList>;
    fn get_all_task_lists(&self) -> Vec<&CardList>;
    fn navigate_left(&mut self, select_last: bool) -> bool;
    fn navigate_right(&mut self, select_last: bool) -> bool;
    fn refresh_lists(&mut self, ctx: &ViewRefreshContext);
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct SingleListLayout {
    task_list: CardList,
}

impl SingleListLayout {
    pub fn new() -> Self {
        Self {
            task_list: CardList::new(CardListId::All),
        }
    }
}

impl Default for SingleListLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutStrategy for SingleListLayout {
    fn get_active_task_list(&self) -> Option<&CardList> {
        Some(&self.task_list)
    }

    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList> {
        Some(&mut self.task_list)
    }

    fn get_all_task_lists(&self) -> Vec<&CardList> {
        vec![&self.task_list]
    }

    fn navigate_left(&mut self, _select_last: bool) -> bool {
        false
    }

    fn navigate_right(&mut self, _select_last: bool) -> bool {
        false
    }

    fn refresh_lists(&mut self, ctx: &ViewRefreshContext) {
        let card_ids = build_query(ctx).execute();
        self.task_list.update_cards(card_ids);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct ColumnListsLayout {
    column_lists: Vec<CardList>,
    active_column_index: usize,
}

impl ColumnListsLayout {
    pub fn new() -> Self {
        Self {
            column_lists: Vec::new(),
            active_column_index: 0,
        }
    }

    pub fn get_active_column_index(&self) -> usize {
        self.active_column_index
    }

    pub fn set_active_column_index(&mut self, index: usize) {
        if index < self.column_lists.len() {
            self.active_column_index = index;
        }
    }
}

impl Default for ColumnListsLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutStrategy for ColumnListsLayout {
    fn get_active_task_list(&self) -> Option<&CardList> {
        self.column_lists.get(self.active_column_index)
    }

    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList> {
        self.column_lists.get_mut(self.active_column_index)
    }

    fn get_all_task_lists(&self) -> Vec<&CardList> {
        self.column_lists.iter().collect()
    }

    fn navigate_left(&mut self, select_last: bool) -> bool {
        if self.active_column_index > 0 {
            self.active_column_index -= 1;
            if let Some(list) = self.get_active_task_list_mut() {
                if list.is_empty() {
                    list.clear();
                } else if select_last {
                    list.set_selected_index(Some(list.len() - 1));
                } else if list.get_selected_index().is_none() {
                    list.set_selected_index(Some(0));
                }
            }
            true
        } else {
            false
        }
    }

    fn navigate_right(&mut self, select_last: bool) -> bool {
        if self.active_column_index < self.column_lists.len().saturating_sub(1) {
            self.active_column_index += 1;
            if let Some(list) = self.get_active_task_list_mut() {
                if list.is_empty() {
                    list.clear();
                } else if select_last {
                    list.set_selected_index(Some(list.len() - 1));
                } else if list.get_selected_index().is_none() {
                    list.set_selected_index(Some(0));
                }
            }
            true
        } else {
            false
        }
    }

    fn refresh_lists(&mut self, ctx: &ViewRefreshContext) {
        let board_columns = sorted_board_columns(ctx.board.id, ctx.all_columns);

        let mut new_column_lists = Vec::new();

        for column in board_columns.iter() {
            let card_ids = build_query(ctx).in_column(column.id).execute();

            let existing_list = self
                .column_lists
                .iter()
                .find(|list| list.id == CardListId::Column(column.id));

            let (prev_selected_card, prev_scroll_offset) = if let Some(existing) = existing_list {
                (
                    existing.get_selected_card_id(),
                    existing.get_scroll_offset(),
                )
            } else {
                (None, 0)
            };

            let mut task_list = CardList::new(CardListId::Column(column.id));
            task_list.update_cards(card_ids);
            if let Some(card_id) = prev_selected_card {
                task_list.select_card(card_id);
            }
            task_list.set_scroll_offset(prev_scroll_offset);
            new_column_lists.push(task_list);
        }

        self.column_lists = new_column_lists;

        if self.active_column_index >= self.column_lists.len() {
            self.active_column_index = self.column_lists.len().saturating_sub(1);
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct VirtualUnifiedLayout {
    unified_list: CardList,
    column_boundaries: Vec<ColumnBoundary>,
}

impl VirtualUnifiedLayout {
    pub fn new() -> Self {
        Self {
            unified_list: CardList::new(CardListId::All),
            column_boundaries: Vec::new(),
        }
    }

    pub fn get_column_boundaries(&self) -> &[ColumnBoundary] {
        &self.column_boundaries
    }
}

impl Default for VirtualUnifiedLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutStrategy for VirtualUnifiedLayout {
    fn get_active_task_list(&self) -> Option<&CardList> {
        Some(&self.unified_list)
    }

    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList> {
        Some(&mut self.unified_list)
    }

    fn get_all_task_lists(&self) -> Vec<&CardList> {
        vec![&self.unified_list]
    }

    fn navigate_left(&mut self, _select_last: bool) -> bool {
        false
    }

    fn navigate_right(&mut self, _select_last: bool) -> bool {
        false
    }

    fn refresh_lists(&mut self, ctx: &ViewRefreshContext) {
        let board_columns = sorted_board_columns(ctx.board.id, ctx.all_columns);

        let mut unified_cards = Vec::new();
        let mut new_boundaries = Vec::new();

        for column in board_columns.iter() {
            let card_ids = build_query(ctx).in_column(column.id).execute();
            let card_count = card_ids.len();

            if card_count > 0 {
                new_boundaries.push(ColumnBoundary {
                    column_id: column.id,
                    column_name: column.name.clone(),
                    start_index: unified_cards.len(),
                    card_count,
                });

                unified_cards.extend(card_ids);
            }
        }

        self.unified_list.update_cards(unified_cards);
        self.column_boundaries = new_boundaries;
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, Card, Column};

    fn make_board() -> Board {
        Board::new("Board", None::<String>)
    }

    fn make_column(board: &Board, name: &str, position: i32) -> Column {
        Column::new(board.id, name, position)
    }

    fn make_card(board: &Board, column: &Column, title: &str, position: i32) -> Card {
        Card::new(board.id, column.id, title, position)
    }

    fn ctx<'a>(
        board: &'a Board,
        cards: &'a [Card],
        columns: &'a [Column],
    ) -> ViewRefreshContext<'a> {
        ViewRefreshContext {
            board,
            all_cards: cards,
            all_columns: columns,
            all_sprints: &[],
            active_sprint_filters: Default::default(),
            hide_assigned_cards: false,
            search_query: None,
        }
    }

    #[test]
    fn test_single_list_layout_navigate_left_and_right_always_report_no_movement() {
        let mut layout = SingleListLayout::new();
        assert!(!layout.navigate_left(false));
        assert!(!layout.navigate_right(false));
    }

    #[test]
    fn test_single_list_layout_get_all_task_lists_returns_exactly_one_list() {
        let layout = SingleListLayout::new();
        assert_eq!(
            layout.get_all_task_lists().len(),
            1,
            "SingleListLayout must expose exactly one list regardless of board content"
        );
    }

    #[test]
    fn test_single_list_layout_refresh_lists_loads_all_matching_cards_across_columns() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let col_b = make_column(&board, "B", 1);
        let card1 = make_card(&board, &col_a, "Card 1", 0);
        let card2 = make_card(&board, &col_b, "Card 2", 0);
        let cards = vec![card1, card2];
        let columns = vec![col_a.clone(), col_b.clone()];

        let mut layout = SingleListLayout::new();
        layout.refresh_lists(&ctx(&board, &cards, &columns));

        assert_eq!(
            layout.get_active_task_list().unwrap().len(),
            2,
            "the single list must contain cards from every column, unlike ColumnListsLayout"
        );
    }

    #[test]
    fn test_single_list_layout_refresh_lists_respects_search_query() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let matching = make_card(&board, &col_a, "Fix bug", 0);
        let non_matching = make_card(&board, &col_a, "Add feature", 1);
        let matching_id = matching.id;
        let cards = vec![matching, non_matching];
        let columns = vec![col_a.clone()];

        let mut layout = SingleListLayout::new();
        let mut search_ctx = ctx(&board, &cards, &columns);
        search_ctx.search_query = Some("bug");
        layout.refresh_lists(&search_ctx);

        let list = layout.get_active_task_list().unwrap();
        assert_eq!(
            list.len(),
            1,
            "only the matching card must survive the search filter"
        );
        assert!(list.cards.contains(&matching_id));
    }

    #[test]
    fn test_column_lists_layout_navigate_right_advances_and_selects_first_card() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let col_b = make_column(&board, "B", 1);

        let mut layout = ColumnListsLayout::new();
        layout.column_lists = vec![
            CardList::with_cards(CardListId::Column(col_a.id), vec![Uuid::new_v4()]),
            CardList::with_cards(CardListId::Column(col_b.id), vec![Uuid::new_v4()]),
        ];

        let moved = layout.navigate_right(false);

        assert!(
            moved,
            "navigate_right must report movement when not at the last column"
        );
        assert_eq!(layout.get_active_column_index(), 1);
        assert_eq!(
            layout.get_active_task_list().unwrap().get_selected_index(),
            Some(0),
            "entering a column with no prior selection must default to the first card"
        );
    }

    #[test]
    fn test_column_lists_layout_navigate_right_at_last_column_does_not_move() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);

        let mut layout = ColumnListsLayout::new();
        layout.column_lists = vec![CardList::new(CardListId::Column(col_a.id))];

        let moved = layout.navigate_right(false);

        assert!(
            !moved,
            "navigate_right at the last column must report no movement"
        );
        assert_eq!(layout.get_active_column_index(), 0);
    }

    #[test]
    fn test_column_lists_layout_navigate_left_at_first_column_does_not_move() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);

        let mut layout = ColumnListsLayout::new();
        layout.column_lists = vec![CardList::new(CardListId::Column(col_a.id))];

        let moved = layout.navigate_left(false);

        assert!(
            !moved,
            "navigate_left at the first column must report no movement"
        );
        assert_eq!(layout.get_active_column_index(), 0);
    }

    #[test]
    fn test_column_lists_layout_navigate_left_with_select_last_selects_final_card() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let col_b = make_column(&board, "B", 1);

        let mut layout = ColumnListsLayout::new();
        layout.column_lists = vec![
            CardList::with_cards(
                CardListId::Column(col_a.id),
                vec![Uuid::new_v4(), Uuid::new_v4()],
            ),
            CardList::new(CardListId::Column(col_b.id)),
        ];
        layout.set_active_column_index(1);

        layout.navigate_left(true);

        assert_eq!(
            layout.get_active_task_list().unwrap().get_selected_index(),
            Some(1),
            "select_last must select the final card in the entered column"
        );
    }

    #[test]
    fn test_column_lists_layout_navigate_into_empty_column_clears_selection() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let col_b = make_column(&board, "B", 1);

        let mut layout = ColumnListsLayout::new();
        layout.column_lists = vec![
            CardList::with_cards(CardListId::Column(col_a.id), vec![Uuid::new_v4()]),
            CardList::new(CardListId::Column(col_b.id)),
        ];

        layout.navigate_right(false);

        assert_eq!(
            layout.get_active_task_list().unwrap().get_selected_index(),
            None,
            "entering an empty column must leave no selection"
        );
    }

    #[test]
    fn test_column_lists_layout_refresh_lists_builds_one_list_per_board_column_in_position_order() {
        let board = make_board();
        let col_a = make_column(&board, "A", 1);
        let col_b = make_column(&board, "B", 0);
        let card_a = make_card(&board, &col_a, "Card A", 0);
        let card_b = make_card(&board, &col_b, "Card B", 0);
        let cards = vec![card_a, card_b];
        let columns = vec![col_a.clone(), col_b.clone()];

        let mut layout = ColumnListsLayout::new();
        layout.refresh_lists(&ctx(&board, &cards, &columns));

        let lists = layout.get_all_task_lists();
        assert_eq!(lists.len(), 2, "one CardList per board column");
        assert_eq!(
            lists[0].id,
            CardListId::Column(col_b.id),
            "columns must be ordered by position, not insertion order"
        );
        assert_eq!(lists[1].id, CardListId::Column(col_a.id));
    }

    #[test]
    fn test_column_lists_layout_refresh_lists_preserves_selection_and_scroll_offset() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let card1 = make_card(&board, &col_a, "Card 1", 0);
        let card2 = make_card(&board, &col_a, "Card 2", 1);
        let card3 = make_card(&board, &col_a, "Card 3", 2);
        let card4 = make_card(&board, &col_a, "Card 4", 3);
        let card1_id = card1.id;
        let cards = vec![card1, card2, card3, card4];
        let columns = vec![col_a.clone()];

        let mut layout = ColumnListsLayout::new();
        layout.refresh_lists(&ctx(&board, &cards, &columns));
        layout
            .get_active_task_list_mut()
            .unwrap()
            .select_card(card1_id);
        layout
            .get_active_task_list_mut()
            .unwrap()
            .set_scroll_offset(3);

        // A second refresh must not silently reset the user's place in the list.
        layout.refresh_lists(&ctx(&board, &cards, &columns));

        let list = layout.get_active_task_list().unwrap();
        assert_eq!(
            list.get_selected_card_id(),
            Some(card1_id),
            "selection must survive a refresh with the same cards"
        );
        assert_eq!(
            list.get_scroll_offset(),
            3,
            "scroll offset must survive a refresh with the same cards"
        );
    }

    #[test]
    fn test_column_lists_layout_refresh_lists_clamps_active_index_when_columns_shrink() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let columns_before = vec![
            col_a.clone(),
            make_column(&board, "B", 1),
            make_column(&board, "C", 2),
        ];

        let mut layout = ColumnListsLayout::new();
        layout.refresh_lists(&ctx(&board, &[], &columns_before));
        layout.set_active_column_index(2);

        let columns_after = vec![col_a.clone()];
        layout.refresh_lists(&ctx(&board, &[], &columns_after));

        assert_eq!(
            layout.get_active_column_index(),
            0,
            "active_column_index must clamp into range when columns shrink"
        );
    }

    #[test]
    fn test_virtual_unified_layout_refresh_lists_skips_empty_columns_and_builds_boundaries() {
        let board = make_board();
        let col_a = make_column(&board, "A", 0);
        let col_b = make_column(&board, "B", 1);
        let col_c = make_column(&board, "C", 2);
        let card1 = make_card(&board, &col_a, "Card 1", 0);
        let card2 = make_card(&board, &col_c, "Card 2", 0);
        let card3 = make_card(&board, &col_c, "Card 3", 1);
        let cards = vec![card1, card2, card3];
        let columns = vec![col_a.clone(), col_b.clone(), col_c.clone()];

        let mut layout = VirtualUnifiedLayout::new();
        layout.refresh_lists(&ctx(&board, &cards, &columns));

        let boundaries = layout.get_column_boundaries();
        assert_eq!(
            boundaries.len(),
            2,
            "the empty middle column must not produce a boundary entry"
        );
        assert_eq!(boundaries[0].column_id, col_a.id);
        assert_eq!(boundaries[0].start_index, 0);
        assert_eq!(boundaries[0].card_count, 1);
        assert_eq!(boundaries[1].column_id, col_c.id);
        assert_eq!(
            boundaries[1].start_index, 1,
            "the second boundary must start after the first column's cards"
        );
        assert_eq!(boundaries[1].card_count, 2);
        assert_eq!(
            layout.get_active_task_list().unwrap().len(),
            3,
            "the unified list must contain all cards across non-empty columns"
        );
    }

    #[test]
    fn test_virtual_unified_layout_navigate_left_and_right_always_report_no_movement() {
        let mut layout = VirtualUnifiedLayout::new();
        assert!(!layout.navigate_left(false));
        assert!(!layout.navigate_right(false));
    }
}
