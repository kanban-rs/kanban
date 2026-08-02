use kanban_backend_memory::InMemoryStore;
use kanban_domain::command_batch::CommandBatch;
use kanban_domain::command_store::CommandStore;
use kanban_domain::commands::{BoardCommand, Command, CreateBoard};
use uuid::Uuid;

fn make_batch(name: &str) -> CommandBatch {
    CommandBatch::from(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: name.into(),
        card_prefix: None,
        position: 0,
    }))])
}

macro_rules! contract_tests {
    ($make_store:expr) => {
        #[test]
        fn test_append_and_load_batches() {
            let store = $make_store;
            store.append_batch(&make_batch("B1")).unwrap();
            store.append_batch(&make_batch("B2")).unwrap();

            let batches = store.load_batches(0, 2).unwrap();
            assert_eq!(batches.len(), 2);
            assert_eq!(batches[0].commands.len(), 1);
            assert_eq!(batches[1].commands.len(), 1);
        }

        #[test]
        fn test_load_batches_half_open_range() {
            let store = $make_store;
            store.append_batch(&make_batch("B1")).unwrap();
            store.append_batch(&make_batch("B2")).unwrap();
            store.append_batch(&make_batch("B3")).unwrap();

            let batches = store.load_batches(1, 3).unwrap();
            assert_eq!(batches.len(), 2);

            let batches = store.load_batches(0, 1).unwrap();
            assert_eq!(batches.len(), 1);
        }
    };
}

mod in_memory {
    use super::*;

    fn make_store() -> InMemoryStore {
        InMemoryStore::new()
    }

    contract_tests!(make_store());
}
