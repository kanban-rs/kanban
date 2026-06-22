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
}
