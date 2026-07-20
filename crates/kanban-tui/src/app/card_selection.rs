use super::{App, SprintTaskPanel};
use kanban_domain::{partition_sprint_cards, sort_card_ids, Card, SortField, SortOrder};

impl App {
    pub fn get_board_card_count(&self, board_id: uuid::Uuid) -> usize {
        let filter = self.board_card_filter(board_id);
        let board = self.model.boards().iter().find(|b| b.id == board_id);
        kanban_domain::count_filtered_cards(
            self.model.cards(),
            self.model.columns(),
            self.model.sprints(),
            board,
            &filter,
        )
    }

    pub fn get_sorted_board_cards(&self, board_id: uuid::Uuid) -> Vec<Card> {
        let filter = self.board_card_filter(board_id);
        let board = self.model.boards().iter().find(|b| b.id == board_id);
        kanban_domain::filter_and_sort_cards(
            self.model.cards(),
            self.model.columns(),
            self.model.sprints(),
            board,
            &filter,
        )
    }

    fn board_card_filter(&self, board_id: uuid::Uuid) -> kanban_domain::CardListFilter {
        let sprint_ids: std::collections::HashSet<uuid::Uuid> =
            self.filter.active_sprint_filters.iter().copied().collect();
        kanban_domain::CardListFilter {
            board_id: Some(board_id),
            sprint_ids: (!sprint_ids.is_empty()).then_some(sprint_ids),
            hide_assigned: self.filter.hide_assigned_cards,
            ..Default::default()
        }
    }

    pub fn get_selected_card_in_context(&self) -> Option<Card> {
        if let Some(task_list) = self.view.strategy.get_active_task_list() {
            if let Some(card_id) = task_list.get_selected_card_id() {
                return self.model.card_by_id(card_id).cloned();
            }
        }
        None
    }

    pub fn get_selected_card_id(&self) -> Option<uuid::Uuid> {
        self.view
            .strategy
            .get_active_task_list()
            .and_then(|list| list.get_selected_card_id())
    }

    pub fn select_card_by_id(&mut self, card_id: uuid::Uuid) {
        // Try the active task list first (covers flat and grouped views, and
        // kanban view when the card stays in the same column).
        if let Some(task_list) = self.view.strategy.get_active_task_list_mut() {
            if task_list.select_card(card_id) {
                return;
            }
        }
        // Kanban (column) view: if the card moved to a different column the
        // active list no longer contains it.  Find the column that now holds
        // the card, switch the active column to it, then select.
        let col_index = self
            .view
            .strategy
            .get_all_task_lists()
            .iter()
            .enumerate()
            .find_map(|(i, list)| list.cards.iter().position(|&id| id == card_id).map(|_| i));
        if let Some(idx) = col_index {
            self.view.strategy.try_navigate_to_column(idx);
            if let Some(task_list) = self.view.strategy.get_active_task_list_mut() {
                task_list.select_card(card_id);
            }
        }
    }

    pub fn get_card_for_detail_view(&self) -> Option<Card> {
        self.selection
            .active_card_id
            .and_then(|id| self.model.card_by_id(id).cloned())
    }

    /// Sets `active_card_id` to `id` if a card with that id exists in the
    /// model. Returns whether the activation took effect, so callers that
    /// gate downstream work on the card existing can chain off the boolean.
    /// On miss the previously-active card is left untouched; sites that
    /// require clear-on-miss semantics must use [`Self::set_active_card_or_clear`].
    pub(crate) fn activate_card(&mut self, id: uuid::Uuid) -> bool {
        if self.model.card_by_id(id).is_some() {
            self.selection.active_card_id = Some(id);
            true
        } else {
            false
        }
    }

    /// Sets `active_card_id` to `id` if the card resolves in the model,
    /// otherwise clears it. Use at sites where `id` was obtained from a
    /// surface that may still reference an archived card (the file-watcher
    /// reload race), so downstream code that gates on
    /// `active_card_id.is_some()` does not act on a stale previous card.
    pub(crate) fn set_active_card_or_clear(&mut self, id: uuid::Uuid) {
        self.selection.active_card_id = self.model.card_by_id(id).map(|c| c.id);
    }

    pub fn populate_sprint_task_lists(&mut self, sprint_id: uuid::Uuid) {
        let cards = self.model.cards();
        let board_opt = self
            .selection
            .active_board_id
            .and_then(|id| self.model.board_by_id(id));

        let (uncompleted_ids, completed_ids) = if let Some(board) = board_opt {
            let columns = self.model.columns();
            let sprints = self.model.sprints();
            let sorted_sprint_ids =
                kanban_domain::CardQueryBuilder::new(cards, columns, sprints, board)
                    .in_sprints(std::iter::once(sprint_id))
                    .execute();
            let mut unc = Vec::new();
            let mut comp = Vec::new();
            for id in sorted_sprint_ids {
                if let Some(card) = cards.iter().find(|c| c.id == id) {
                    if card.is_completed() {
                        comp.push(id);
                    } else {
                        unc.push(id);
                    }
                }
            }
            (unc, comp)
        } else {
            partition_sprint_cards(sprint_id, cards)
        };

        self.sprint_view
            .uncompleted_cards
            .update_cards(uncompleted_ids.clone());
        self.sprint_view
            .completed_cards
            .update_cards(completed_ids.clone());

        self.sprint_view
            .uncompleted_component
            .update_cards(uncompleted_ids);
        self.sprint_view
            .completed_component
            .update_cards(completed_ids);

        // Default to uncompleted panel
        self.sprint_view.panel = SprintTaskPanel::Uncompleted;
    }

    pub fn apply_sort_to_sprint_lists(&mut self, sort_field: SortField, sort_order: SortOrder) {
        let cards = self.model.cards();
        let sorted_uncompleted_ids = sort_card_ids(
            &self.sprint_view.uncompleted_cards.cards,
            cards,
            sort_field,
            sort_order,
        );
        let sorted_completed_ids = sort_card_ids(
            &self.sprint_view.completed_cards.cards,
            cards,
            sort_field,
            sort_order,
        );

        self.sprint_view
            .uncompleted_cards
            .update_cards(sorted_uncompleted_ids);
        self.sprint_view
            .completed_cards
            .update_cards(sorted_completed_ids);

        self.sprint_view
            .uncompleted_component
            .update_cards(self.sprint_view.uncompleted_cards.cards.clone());
        self.sprint_view
            .completed_component
            .update_cards(self.sprint_view.completed_cards.cards.clone());
    }
}

#[cfg(test)]
mod active_card_helpers {
    use crate::App;
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};

    fn app_with_card() -> (App, uuid::Uuid) {
        let mut app = App::test_default();
        let board = app.ctx.create_board("B".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "Todo".into(), None)
            .unwrap();
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "C".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let snap = Snapshot {
            archived_boards: Vec::new(),
            boards: app.ctx.data_store().list_boards().unwrap(),
            columns: app.ctx.data_store().list_all_columns().unwrap(),
            cards: app.ctx.data_store().list_all_cards().unwrap(),
            archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
            sprints: app.ctx.data_store().list_all_sprints().unwrap(),
            graph: app.ctx.data_store().get_graph().unwrap(),
        };
        app.model.load_from_snapshot(snap);
        (app, card.id)
    }

    #[test]
    fn test_activate_card_with_known_id_sets_active_card_id_and_returns_true() {
        let (mut app, card_id) = app_with_card();

        let succeeded = app.activate_card(card_id);

        assert!(succeeded, "must report success when the card exists");
        assert_eq!(app.selection.active_card_id, Some(card_id));
    }

    #[test]
    fn test_activate_card_with_unknown_id_preserves_active_card_id_and_returns_false() {
        let (mut app, card_id) = app_with_card();
        app.selection.active_card_id = Some(card_id);

        let succeeded = app.activate_card(uuid::Uuid::new_v4());

        assert!(!succeeded, "must report failure when the card is absent");
        assert_eq!(
                app.selection.active_card_id,
                Some(card_id),
                "activate_card must not touch active_card_id on miss — sites that need clear-on-miss must use set_active_card_or_clear"
            );
    }

    #[test]
    fn test_set_active_card_or_clear_with_known_id_sets_active_card_id() {
        let (mut app, card_id) = app_with_card();

        app.set_active_card_or_clear(card_id);

        assert_eq!(app.selection.active_card_id, Some(card_id));
    }

    #[test]
    fn test_set_active_card_or_clear_with_unknown_id_clears_active_card_id() {
        let (mut app, card_id) = app_with_card();
        app.selection.active_card_id = Some(card_id);

        app.set_active_card_or_clear(uuid::Uuid::new_v4());

        assert_eq!(
                app.selection.active_card_id, None,
                "set_active_card_or_clear must clear the previous active card when the new id is absent — prevents downstream handlers from acting on a stale active card"
            );
    }
}
