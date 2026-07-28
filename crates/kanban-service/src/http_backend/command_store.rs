use kanban_domain::command_batch::CommandBatch;
use kanban_domain::command_store::CommandStore;
use kanban_domain::KanbanResult;

use super::HttpBackend;

impl CommandStore for HttpBackend {
    fn append_batch(&self, _batch: &CommandBatch) -> KanbanResult<u64> {
        Err(kanban_domain::KanbanError::unsupported("append_batch"))
    }

    fn batch_count(&self) -> KanbanResult<u64> {
        Err(kanban_domain::KanbanError::unsupported("batch_count"))
    }

    fn load_batches(&self, _from: u64, _to: u64) -> KanbanResult<Vec<CommandBatch>> {
        Err(kanban_domain::KanbanError::unsupported("load_batches"))
    }
}
