use super::KanbanContext;
use kanban_domain::commands::{BoardCommand, Command};
use kanban_domain::{Board, BoardUpdate, KanbanError, KanbanResult};
use uuid::Uuid;

impl KanbanContext {
    pub(super) fn create_board_impl(
        &mut self,
        name: String,
        card_prefix: Option<String>,
    ) -> KanbanResult<Board> {
        use kanban_domain::commands::CreateBoard;
        let id = Uuid::new_v4();
        let position = self.backend.list_boards()?.len() as i32;
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id,
            name,
            card_prefix,
            position,
        }));
        self.execute(vec![cmd])?;
        self.get_board_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Board creation succeeded but board not found".into())
        })
    }

    pub(super) fn list_boards_impl(&self) -> KanbanResult<Vec<Board>> {
        self.backend.list_boards()
    }

    pub(super) fn get_board_impl(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.backend.get_board(id)
    }

    pub(super) fn update_board_impl(
        &mut self,
        id: Uuid,
        updates: BoardUpdate,
    ) -> KanbanResult<Board> {
        use kanban_domain::commands::UpdateBoard;
        let cmd = Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_board_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Board", id))
    }

    pub(super) fn delete_board_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        let commands = crate::cascade::delete_board(self.backend.as_data_store(), id)?;
        self.execute(commands)
    }
}
