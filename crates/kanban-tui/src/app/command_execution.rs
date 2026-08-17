use super::App;
use kanban_domain::KanbanResult;

impl App {
    /// Execute a single command and queue a flush.
    /// For multiple commands, prefer `execute_commands_batch` to produce only one flush signal.
    pub fn execute_command(
        &mut self,
        command: kanban_domain::commands::Command,
    ) -> KanbanResult<()> {
        self.execute_commands_batch(vec![command])
    }

    /// Execute multiple commands as a batch, producing a single flush signal.
    pub fn execute_commands_batch(
        &mut self,
        commands: Vec<kanban_domain::commands::Command>,
    ) -> KanbanResult<()> {
        self.ctx.execute_commands_batch(commands)?;
        Ok(())
    }

    /// Like `execute_commands_batch`, but the batch is built inside the
    /// transaction so anything the builder writes (e.g. allocating a card
    /// number) rolls back with it. See `KanbanContext::execute_with`.
    pub fn execute_with(
        &mut self,
        build: impl FnOnce(
            &dyn kanban_domain::DataStore,
        ) -> KanbanResult<Vec<kanban_domain::commands::Command>>,
    ) -> KanbanResult<()> {
        self.ctx.execute_with(build)?;
        Ok(())
    }
}
