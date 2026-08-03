use crate::view_strategy::ViewRefreshContext;
use kanban_domain::card_lifecycle::sorted_board_columns;
use kanban_domain::CardQueryBuilder;
use kanban_view::card_list::{CardList, CardListId};
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
