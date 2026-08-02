use super::{App, AppMode};

impl App {
    pub fn get_current_priority_selection_index(&self) -> usize {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card_by_id(active_id) {
                use kanban_domain::CardPriority;
                return match card.priority {
                    CardPriority::Low => 0,
                    CardPriority::Medium => 1,
                    CardPriority::High => 2,
                    CardPriority::Critical => 3,
                };
            }
        }
        0
    }

    pub fn get_current_sprint_selection_index(&self) -> usize {
        use crate::components::sprint_assign_list::{build_entries, sprint_id_of};

        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card_by_id(active_id) {
                if let Some(card_sprint_id) = card.sprint_id {
                    if let Some(board) = self.active_board() {
                        let sprints = self.model.sprints();
                        let entries = build_entries(sprints, board.id, chrono::Utc::now());
                        for (idx, entry) in entries.iter().enumerate() {
                            if sprint_id_of(entry) == Some(card_sprint_id) {
                                return idx;
                            }
                        }
                    }
                }
            }
        }
        0
    }

    pub fn get_current_sort_field_selection_index(&self) -> usize {
        self.filter
            .current_sort_field
            .map(crate::components::selection_dialog::popup_index_of_sort_field)
            .unwrap_or(0)
    }

    /// The picker row for the projects-panel sort field: the popup index of the
    /// model's current board-list `BoardSortField`. Mirrors the card variant.
    pub fn get_current_board_sort_field_selection_index(&self) -> usize {
        let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
        let (field, _order) = self.model.board_sort(want_archived);
        crate::components::selection_dialog::popup_index_of_board_sort_field(field)
    }
}

#[cfg(test)]
mod active_card_index_regression {
    use crate::test_helpers::{load_with_card_order, setup_reload_resort_fixture};
    use crate::App;
    use kanban_domain::{CardPriority, CardUpdate, KanbanOperations};

    #[test]
    fn test_get_current_priority_selection_index_after_reload_resort_returns_originally_selected_card_priority(
    ) {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        app.ctx
            .update_card(
                fx.a_id,
                CardUpdate {
                    priority: Some(CardPriority::Critical),
                    ..Default::default()
                },
            )
            .unwrap();
        app.ctx
            .update_card(
                fx.p_id,
                CardUpdate {
                    priority: Some(CardPriority::Low),
                    ..Default::default()
                },
            )
            .unwrap();
        load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

        let idx = app.get_current_priority_selection_index();

        assert_eq!(
                idx, 3,
                "must return Critical's index (3) — A's priority — not Low's index (0) which is P's priority at A's stale index"
            );
    }

    #[test]
    fn test_get_current_sprint_selection_index_after_reload_resort_returns_originally_selected_card_sprint(
    ) {
        use crate::components::sprint_assign_list::{build_entries, sprint_id_of};

        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        let sprint_a = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
        let sprint_p = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
        app.ctx.assign_card_to_sprint(fx.a_id, sprint_a.id).unwrap();
        app.ctx.assign_card_to_sprint(fx.p_id, sprint_p.id).unwrap();
        load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

        let sprints = app.model.sprints().to_vec();
        let entries = build_entries(&sprints, fx.board_id, chrono::Utc::now());
        let expected_idx = entries
            .iter()
            .position(|e| sprint_id_of(e) == Some(sprint_a.id))
            .expect("sprint_a appears in entries");

        let idx = app.get_current_sprint_selection_index();

        assert_eq!(
            idx, expected_idx,
            "must return A's sprint index, not P's sprint index at A's stale slot"
        );
    }
}
