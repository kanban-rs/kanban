use kanban_domain::command_envelope::CommandEnvelope;
use kanban_domain::command_store::CommandStore;
use kanban_domain::commands::{BoardCommand, Command, CreateBoard};
use kanban_domain::InMemoryStore;
use uuid::Uuid;

fn make_cmd(name: &str) -> CommandEnvelope {
    CommandEnvelope::from(Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: name.into(),
        card_prefix: None,
        position: 0,
    })))
}

macro_rules! contract_tests {
    ($make_store:expr) => {
        #[test]
        fn test_append_and_load_commands() {
            let store = $make_store;
            store.append_commands(&[make_cmd("B1")]).unwrap();
            store.append_commands(&[make_cmd("B2")]).unwrap();

            let batches = store.load_commands(0, 2).unwrap();
            assert_eq!(batches.len(), 2);
            assert_eq!(batches[0].len(), 1);
            assert_eq!(batches[1].len(), 1);
        }

        #[test]
        fn test_load_commands_half_open_range() {
            let store = $make_store;
            store.append_commands(&[make_cmd("B1")]).unwrap();
            store.append_commands(&[make_cmd("B2")]).unwrap();
            store.append_commands(&[make_cmd("B3")]).unwrap();

            let batches = store.load_commands(1, 3).unwrap();
            assert_eq!(batches.len(), 2);

            let batches = store.load_commands(0, 1).unwrap();
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
