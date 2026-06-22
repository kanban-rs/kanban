use super::KanbanContext;
use kanban_domain::commands::{Command, SprintCommand};
use kanban_domain::{Board, DataStore, KanbanError, KanbanResult, Snapshot, Sprint, SprintUpdate};
use kanban_persistence::PersistenceError;
use uuid::Uuid;

impl KanbanContext {
    pub(super) fn carry_over_sprint_cards_impl(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        use kanban_domain::query::sprint::get_sprint_uncompleted_cards;

        let from_sprint = self
            .get_sprint_impl(from_sprint_id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", from_sprint_id))?;
        if from_sprint.status != kanban_domain::SprintStatus::Completed
            && from_sprint.status != kanban_domain::SprintStatus::Cancelled
        {
            return Err(KanbanError::validation(format!(
                "Source sprint must be Completed or Cancelled, got {:?}",
                from_sprint.status
            )));
        }
        let to_sprint = self
            .get_sprint_impl(to_sprint_id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", to_sprint_id))?;
        if to_sprint.status != kanban_domain::SprintStatus::Planning {
            return Err(KanbanError::validation(format!(
                "Target sprint must be Planning, got {:?}",
                to_sprint.status
            )));
        }

        let all_cards = self.backend.list_all_cards()?;
        let ids: Vec<Uuid> = get_sprint_uncompleted_cards(from_sprint_id, &all_cards)
            .iter()
            .map(|c| c.id)
            .collect();
        self.assign_cards_to_sprint_impl(ids, to_sprint_id)
    }

    pub(super) fn create_sprint_impl(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CreateSprint;

        let default_sprint_prefix = self
            .app_config
            .effective_default_sprint_prefix()
            .to_string();

        let id = Uuid::new_v4();
        let cmd = Command::Sprint(SprintCommand::Create(CreateSprint {
            id,
            board_id,
            name,
            default_sprint_prefix,
            explicit_prefix: prefix,
            auto_consume_name: false,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Sprint creation succeeded but sprint not found".into())
        })
    }

    pub(super) fn list_sprints_impl(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.backend.list_sprints_by_board(board_id)
    }

    pub(super) fn get_sprint_impl(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.backend.get_sprint(id)
    }

    pub(super) fn update_sprint_impl(
        &mut self,
        id: Uuid,
        updates: SprintUpdate,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::UpdateSprint;
        let cmd = Command::Sprint(SprintCommand::Update(UpdateSprint {
            sprint_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn activate_sprint_impl(
        &mut self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::ActivateSprint;
        let duration = duration_days.unwrap_or(14) as u32;
        let cmd = Command::Sprint(SprintCommand::Activate(ActivateSprint {
            sprint_id: id,
            duration_days: duration,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn complete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CompleteSprint;
        let cmd = Command::Sprint(SprintCommand::Complete(CompleteSprint { sprint_id: id }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn cancel_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CancelSprint;
        let cmd = Command::Sprint(SprintCommand::Cancel(CancelSprint { sprint_id: id }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn delete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteSprint;
        let cmd = Command::Sprint(SprintCommand::Delete(DeleteSprint {
            sprint_id: id,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])
    }

    pub(super) fn export_board_impl(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        let snapshot = if let Some(id) = board_id {
            let boards: Vec<_> = self
                .backend
                .list_boards()?
                .into_iter()
                .filter(|b| b.id == id)
                .collect();
            let columns = self.backend.list_columns_by_board(id)?;
            let column_ids: Vec<_> = columns.iter().map(|c| c.id).collect();
            let cards: Vec<_> = self
                .backend
                .list_all_cards()?
                .into_iter()
                .filter(|c| column_ids.contains(&c.column_id))
                .collect();
            let sprints = self.backend.list_sprints_by_board(id)?;
            let graph = self.backend.get_graph()?;
            Snapshot {
                boards,
                columns,
                cards,
                archived_cards: vec![],
                sprints,
                graph,
            }
        } else {
            self.backend.snapshot()?
        };

        serde_json::to_string_pretty(&snapshot)
            .map_err(|e| PersistenceError::Serialization(e.to_string()).into())
    }

    pub(super) fn import_board_impl(&mut self, data: &str) -> KanbanResult<Board> {
        use kanban_domain::commands::BoardCommand;
        use kanban_domain::commands::{Command, CommandContext, ImportEntities};
        use std::collections::HashSet;

        let imported: Snapshot = serde_json::from_str(data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let board = imported
            .boards
            .first()
            .cloned()
            .ok_or_else(|| KanbanError::validation("No board in import data"))?;

        let imported_column_ids: HashSet<Uuid> = imported.columns.iter().map(|c| c.id).collect();
        let existing_column_ids: HashSet<Uuid> = self
            .backend
            .list_all_columns()?
            .into_iter()
            .map(|c| c.id)
            .collect();
        for card in &imported.cards {
            if !imported_column_ids.contains(&card.column_id)
                && !existing_column_ids.contains(&card.column_id)
            {
                return Err(KanbanError::validation(format!(
                    "Card '{}' references column {} which does not exist in the import or the current store",
                    card.title, card.column_id
                )));
            }
        }

        let commands = vec![Command::Board(BoardCommand::Import(ImportEntities {
            boards: imported.boards,
            columns: imported.columns,
            cards: imported.cards,
            archived_cards: imported.archived_cards,
            sprints: imported.sprints,
            graph: Some(imported.graph),
        }))];

        {
            let store: &dyn DataStore = self.backend.as_data_store();
            let ctx = CommandContext { store };
            for cmd in &commands {
                cmd.execute(&ctx)?;
            }
        }

        self.undo_stack.clear();
        self.dirty = true;

        Ok(board)
    }
}
