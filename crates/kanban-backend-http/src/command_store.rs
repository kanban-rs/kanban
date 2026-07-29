use crate::HttpBackend;
use kanban_domain::{CommandBatch, CommandStore, KanbanError, KanbanResult};

impl CommandStore for HttpBackend {
    fn append_batch(&self, _batch: &CommandBatch) -> KanbanResult<u64> {
        Err(KanbanError::unsupported("append_batch"))
    }

    fn batch_count(&self) -> KanbanResult<u64> {
        Err(KanbanError::unsupported("batch_count"))
    }

    fn load_batches(&self, _offset: u64, _limit: u64) -> KanbanResult<Vec<CommandBatch>> {
        Err(KanbanError::unsupported("load_batches"))
    }
}
