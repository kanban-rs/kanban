use super::InMemoryStore;
use crate::command_batch::CommandBatch;
use crate::command_store::CommandStore;
use crate::KanbanResult;

impl CommandStore for InMemoryStore {
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
        let mut log = self.write_log()?;
        log.push(batch.clone());
        Ok(log.len() as u64)
    }

    fn batch_count(&self) -> KanbanResult<u64> {
        Ok(self.read_log()?.len() as u64)
    }

    fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>> {
        let log = self.read_log()?;
        let from = (from as usize).min(log.len());
        let to = (to as usize).min(log.len()).max(from);
        Ok(log[from..to].to_vec())
    }

    fn load_all_batches(&self) -> KanbanResult<(Vec<CommandBatch>, u64)> {
        let log = self.read_log()?;
        Ok((log.clone(), log.len() as u64))
    }
}
