use crate::app::{App, BoardFocus, DialogMode};
use kanban_domain::commands::{
    ActivateSprint, BoardCommand, Command, CompleteSprint, CreateSprint, SprintCommand, UpdateBoard,
};
use kanban_domain::{BoardUpdate, FieldUpdate, SprintStatus};
use uuid::Uuid;

impl App {
    pub fn handle_create_sprint_key(&mut self) {
        if self.focus.board_focus == BoardFocus::Sprints && self.active_board().is_some() {
            self.open_dialog(DialogMode::CreateSprint);
            self.input.clear();
        }
    }

    pub fn handle_activate_sprint_key(&mut self) {
        if let Some(sprint_idx) = self.selection.active_sprint_index {
            // Collect sprint info before mutations
            let sprint_info = {
                let context_board = self.board_in_context();
                if let (Some(sprint), Some(board)) =
                    (self.model.sprints().get(sprint_idx), context_board)
                {
                    if sprint.status == SprintStatus::Planning {
                        Some((sprint.id, sprint.formatted_name(board, "sprint")))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((sprint_id, sprint_name)) = sprint_info {
                {
                    if let Some(board) = self.board_in_context() {
                        let duration = board.sprint_duration_days.unwrap_or(14);
                        let board_id = board.id;

                        // Execute ActivateSprint and UpdateBoard as batch
                        let activate_cmd =
                            Command::Sprint(SprintCommand::Activate(ActivateSprint {
                                sprint_id,
                                duration_days: duration,
                            }));

                        let board_cmd = Command::Board(BoardCommand::Update(UpdateBoard {
                            board_id,
                            updates: BoardUpdate {
                                active_sprint_id: FieldUpdate::Set(sprint_id),
                                ..Default::default()
                            },
                        }));

                        if let Err(e) = self.execute_commands_batch(vec![activate_cmd, board_cmd]) {
                            tracing::error!("Failed to activate sprint: {}", e);
                            self.set_error(format!("Failed to activate sprint: {}", e));
                            return;
                        }
                        self.reload_model();

                        tracing::info!("Activated sprint: {}", sprint_name);
                    }
                }
            }
        }
    }

    pub fn handle_complete_sprint_key(&mut self) {
        if let Some(sprint_idx) = self.selection.active_sprint_index {
            // Collect sprint and board info before mutations
            let sprint_info = {
                let context_board = self.board_in_context();
                if let (Some(sprint), Some(board)) =
                    (self.model.sprints().get(sprint_idx), context_board)
                {
                    if sprint.status == SprintStatus::Active
                        || sprint.status == SprintStatus::Planning
                    {
                        Some((sprint.id, board.id, sprint.formatted_name(board, "sprint")))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((sprint_id, board_id, sprint_name)) = sprint_info {
                // Execute CompleteSprint and UpdateBoard as batch
                let complete_cmd =
                    Command::Sprint(SprintCommand::Complete(CompleteSprint { sprint_id }));

                let board_cmd = Command::Board(BoardCommand::Update(UpdateBoard {
                    board_id,
                    updates: BoardUpdate {
                        active_sprint_id: FieldUpdate::Clear,
                        ..Default::default()
                    },
                }));

                if let Err(e) = self.execute_commands_batch(vec![complete_cmd, board_cmd]) {
                    tracing::error!("Failed to complete sprint: {}", e);
                    self.set_error(format!("Failed to complete sprint: {}", e));
                    return;
                }
                self.reload_model();

                self.filter.active_sprint_filters.remove(&sprint_id);

                tracing::info!("Completed sprint: {}", sprint_name);

                self.pop_mode();
                self.focus.board_focus = BoardFocus::Sprints;
                self.selection.active_sprint_index = None;

                {
                    use kanban_domain::query::sprint::get_sprint_uncompleted_cards;
                    let has_planning = self.model.sprints().iter().any(|s| {
                        s.board_id == board_id
                            && s.status == SprintStatus::Planning
                            && s.id != sprint_id
                    });

                    if has_planning
                        && !get_sprint_uncompleted_cards(sprint_id, self.model.live_cards())
                            .is_empty()
                    {
                        self.dialog_input.carry_over_source_sprint_id = Some(sprint_id);
                        self.dialog_input.carry_over_sprint_selection.set(Some(0));
                        self.open_dialog(DialogMode::CarryOverSprint);
                    }
                }
            }
        }
    }

    pub fn handle_carry_over_for_sprint(&mut self, from_sprint_id: Uuid) {
        let board_id = match self.model.sprints().iter().find(|s| s.id == from_sprint_id) {
            Some(sprint) => sprint.board_id,
            None => return,
        };

        let has_planning_sprint = self
            .model
            .sprints()
            .iter()
            .any(|s| s.board_id == board_id && s.status == SprintStatus::Planning);

        if has_planning_sprint {
            self.dialog_input.carry_over_source_sprint_id = Some(from_sprint_id);
            self.dialog_input.carry_over_sprint_selection.set(Some(0));
            self.open_dialog(DialogMode::CarryOverSprint);
        } else {
            self.set_error("No Planning sprint available for carry-over");
        }
    }

    pub fn create_sprint(&mut self) {
        {
            let (board_id, name) = {
                if let Some(board) = self.board_in_context() {
                    let input_text = self.input.as_str().trim();
                    let name = if input_text.is_empty() {
                        None
                    } else {
                        Some(input_text.to_string())
                    };
                    (board.id, name)
                } else {
                    return;
                }
            };

            let default_sprint_prefix = self
                .app_config
                .effective_default_sprint_prefix()
                .to_string();

            let sprint_id = uuid::Uuid::new_v4();
            let prior_sprint_count = self
                .model
                .sprints()
                .iter()
                .filter(|s| s.board_id == board_id)
                .count();

            let cmd = Command::Sprint(SprintCommand::Create(CreateSprint {
                id: sprint_id,
                board_id,
                name,
                default_sprint_prefix: default_sprint_prefix.clone(),
                explicit_prefix: None,
                auto_consume_name: true,
            }));

            if let Err(e) = self.execute_command(cmd) {
                tracing::error!("Failed to create sprint: {}", e);
                self.set_error(format!("Failed to create sprint: {}", e));
                return;
            }
            self.reload_model();

            tracing::info!("Created sprint (id: {})", sprint_id);

            self.selection.sprint.set(Some(prior_sprint_count));
        }
    }
}

#[cfg(test)]
mod create_sprint_factory_tests {
    use crate::App;
    use kanban_domain::{KanbanOperations, SprintStatus};

    /// Refresh the TUI model from the store so the create handler (which reads
    /// `self.model`) sees prior writes. The event loop does this each frame via
    /// `prepare_frame`; tests pull the snapshot directly.
    fn refresh(app: &mut App) {
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
    }

    /// Seed a board through the service, then point the TUI's active selection
    /// at it so `create_sprint` has a board to mint against.
    fn seed_active_board(app: &mut App) {
        app.ctx
            .create_board("Board".into(), Some("KAN".into()))
            .unwrap();
        refresh(app);
        app.selection.active_board_id = app.model.boards().first().map(|b| b.id);
    }

    /// KAN-798: the TUI sprint-create entry point funnels through the Sprint
    /// factory (`Sprint::create` via the `CreateSprint` command), so a created
    /// sprint carries the factory-seeded lifecycle defaults (Planning status)
    /// and the board-minted user-facing `sprint_number` (1 for the first
    /// sprint), rather than diverging from a hand-assembled command's defaults.
    #[test]
    fn test_tui_create_sprint_funnels_through_factory() {
        let mut app = App::test_default();
        seed_active_board(&mut app);

        app.input.set("Alpha".to_string());
        app.create_sprint();
        app.input.clear();

        let sprints = app.ctx.data_store().list_all_sprints().unwrap();
        assert_eq!(sprints.len(), 1, "exactly one sprint created");
        let sprint = &sprints[0];
        // Factory-seeded lifecycle default.
        assert_eq!(sprint.status, SprintStatus::Planning);
        // Board-minted user-facing number.
        assert_eq!(sprint.sprint_number, 1);
        // Factory uses one clock for both timestamps at create.
        assert_eq!(sprint.created_at, sprint.updated_at);
    }

    /// The TUI create handler still passes `auto_consume_name = true` (a
    /// TUI-only behaviour): with a typed name it allocates that name from the
    /// board pool, and the factory resolves it back on read. This pins that the
    /// widened input preserves the name-source flag end to end.
    #[test]
    fn test_tui_create_sprint_allocates_typed_name_through_factory() {
        let mut app = App::test_default();
        seed_active_board(&mut app);

        app.input.set("Alpha".to_string());
        app.create_sprint();
        app.input.clear();

        let board = app.ctx.list_boards().unwrap().remove(0);
        let sprint = app.ctx.data_store().list_all_sprints().unwrap().remove(0);
        assert_eq!(
            sprint.get_name(&board),
            Some("Alpha"),
            "typed name allocated against the board pool and resolved on read"
        );
    }
}
