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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_all_command_store_methods_return_ok_not_panic() {
        use crate::command_batch::CommandBatch;
        use crate::commands::{BoardCommand, Command, CreateBoard};
        let store = InMemoryStore::new();
        let batch = CommandBatch::from(vec![Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: "B".into(),
            card_prefix: None,
            position: 0,
        }))]);

        assert!(store.batch_count().is_ok());
        assert_eq!(store.batch_count().unwrap(), 0);

        assert!(store.append_batch(&batch).is_ok());
        assert_eq!(store.batch_count().unwrap(), 1);

        assert!(store.load_batches(0, 1).is_ok());
        assert_eq!(store.load_batches(0, 1).unwrap().len(), 1);
    }

    #[test]
    fn test_load_batches_from_beyond_end_returns_empty() {
        use crate::command_batch::CommandBatch;
        let store = InMemoryStore::new();
        let make_batch = || {
            CommandBatch::from(vec![crate::commands::Command::Board(
                crate::commands::BoardCommand::Delete(crate::commands::DeleteBoard {
                    board_id: Uuid::new_v4(),
                }),
            )])
        };
        store.append_batch(&make_batch()).unwrap();
        store.append_batch(&make_batch()).unwrap();
        store.append_batch(&make_batch()).unwrap();

        let result = store.load_batches(10, 20).unwrap();
        assert!(
            result.is_empty(),
            "Expected empty vec for out-of-bounds range"
        );
    }

    #[test]
    fn test_append_batch_stores_full_batch_including_provenance() {
        // The batch is the audit log: the full CommandBatch is stored, with
        // commands AND provenance (issued_by, correlation_id, session_id)
        // preserved on write — nothing is stripped.
        use crate::command_batch::CommandBatch;
        use crate::commands::{BoardCommand, Command, CreateBoard};
        use kanban_core::ClientId;

        let store = InMemoryStore::new();
        let client = ClientId::new();
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: "Test".into(),
            card_prefix: None,
            position: 0,
        }));
        let batch = CommandBatch::wrap(vec![cmd.clone()], client);
        let expected_correlation = batch.correlation_id;
        let expected_session = batch.session_id;

        store.append_batch(&batch).unwrap();
        let loaded = store.load_batches(0, 1).unwrap();

        assert_eq!(loaded.len(), 1);
        let loaded = &loaded[0];
        assert_eq!(loaded.commands, vec![cmd]);
        assert_eq!(
            loaded.issued_by, client,
            "issued_by must survive — provenance is no longer stripped"
        );
        assert_eq!(loaded.correlation_id, expected_correlation);
        assert_eq!(loaded.session_id, expected_session);
        assert_eq!(
            loaded.app_type, batch.app_type,
            "app_type must survive the batch round-trip"
        );
    }

    #[test]
    fn test_load_batches_inverted_range_returns_empty() {
        // An inverted range (from > to) must yield an empty slice, never panic
        // on `log[from..to]`.
        use crate::command_batch::CommandBatch;
        use crate::commands::{BoardCommand, Command, CreateBoard};

        let store = InMemoryStore::new();
        let make_cmd = |name: &str| {
            Command::Board(BoardCommand::Create(CreateBoard {
                id: Uuid::new_v4(),
                name: name.into(),
                card_prefix: None,
                position: 0,
            }))
        };
        store
            .append_batch(&CommandBatch::from(vec![make_cmd("B1")]))
            .unwrap();
        store
            .append_batch(&CommandBatch::from(vec![make_cmd("B2")]))
            .unwrap();

        let loaded = store.load_batches(2, 1).unwrap();
        assert!(
            loaded.is_empty(),
            "inverted range must return empty, not panic"
        );
    }

    #[test]
    fn test_append_batch_preserves_batch_boundaries() {
        use crate::command_batch::CommandBatch;
        use crate::commands::{BoardCommand, Command, CreateBoard};

        let store = InMemoryStore::new();
        let make_cmd = |name: &str| {
            Command::Board(BoardCommand::Create(CreateBoard {
                id: Uuid::new_v4(),
                name: name.into(),
                card_prefix: None,
                position: 0,
            }))
        };

        store
            .append_batch(&CommandBatch::from(vec![make_cmd("B1"), make_cmd("B2")]))
            .unwrap();
        store
            .append_batch(&CommandBatch::from(vec![make_cmd("B3")]))
            .unwrap();

        let batches = store.load_batches(0, 2).unwrap();
        assert_eq!(batches.len(), 2, "two separate append calls = two batches");
        assert_eq!(batches[0].commands.len(), 2, "first batch had 2 commands");
        assert_eq!(batches[1].commands.len(), 1, "second batch had 1 command");
    }
}
