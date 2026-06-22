use super::KanbanContext;
use kanban_domain::commands::{ColumnCommand, Command};
use kanban_domain::{Column, ColumnUpdate, FieldUpdate, KanbanError, KanbanResult};
use uuid::Uuid;

impl KanbanContext {
    pub(super) fn create_column_impl(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        use kanban_domain::commands::CreateColumn;
        let position = match position {
            Some(p) => p,
            None => self.backend.list_columns_by_board(board_id)?.len() as i32,
        };
        let id = Uuid::new_v4();
        let cmd = Command::Column(ColumnCommand::Create(CreateColumn {
            id,
            board_id,
            name,
            position,
        }));
        self.execute(vec![cmd])?;
        self.get_column_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Column creation succeeded but column not found".into())
        })
    }

    pub(super) fn list_columns_impl(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.backend.list_columns_by_board(board_id)
    }

    pub(super) fn get_column_impl(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.backend.get_column(id)
    }

    pub(super) fn update_column_impl(
        &mut self,
        id: Uuid,
        updates: ColumnUpdate,
    ) -> KanbanResult<Column> {
        use kanban_domain::commands::UpdateColumn;
        let cmd = Command::Column(ColumnCommand::Update(UpdateColumn {
            column_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_column_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Column", id))
    }

    pub(super) fn delete_column_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteColumn;
        let cmd = Command::Column(ColumnCommand::Delete(DeleteColumn { column_id: id }));
        self.execute(vec![cmd])
    }

    pub(super) fn reorder_column_impl(
        &mut self,
        id: Uuid,
        new_position: i32,
    ) -> KanbanResult<Column> {
        let updates = ColumnUpdate {
            name: None,
            position: Some(new_position),
            wip_limit: FieldUpdate::NoChange,
        };
        self.update_column_impl(id, updates)
    }
}
