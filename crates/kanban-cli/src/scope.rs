use kanban_service::{requestable, FetchPlan, FetchRound, LoadedEntities};
use uuid::Uuid;

use crate::cli::{BoardAction, Commands, RelationAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ref {
    Id,
    Name,
}

impl Ref {
    fn of(raw: &str) -> Self {
        if Uuid::parse_str(raw).is_ok() {
            Ref::Id
        } else {
            Ref::Name
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CommandScope {
    board: Option<Ref>,
    wants_graph: bool,
}

impl CommandScope {
    pub(crate) fn from_command(cmd: &Commands) -> Self {
        let mut scope = CommandScope::default();

        match cmd {
            Commands::Board(board_cmd) => match &board_cmd.action {
                BoardAction::Get { board }
                | BoardAction::Delete { board }
                | BoardAction::Archive { board } => scope.board = Some(Ref::of(board)),
                BoardAction::Update(args) => scope.board = Some(Ref::of(&args.board)),
                BoardAction::Create { .. }
                | BoardAction::List { .. }
                | BoardAction::Restore { .. }
                | BoardAction::DeleteArchived { .. }
                | BoardAction::SetSort { .. } => {}
            },
            Commands::Column(column_cmd) => match &column_cmd.action {
                crate::cli::ColumnAction::Create { board, .. }
                | crate::cli::ColumnAction::List { board, .. } => {
                    scope.board = Some(Ref::of(board));
                }
                _ => {}
            },
            Commands::Card(card_cmd) => match &card_cmd.action {
                crate::cli::CardAction::Create(args) => {
                    scope.board = Some(Ref::of(&args.board));
                }
                crate::cli::CardAction::List(args) => {
                    scope.board = args.board.as_deref().map(Ref::of);
                }
                _ => {}
            },
            Commands::Sprint(sprint_cmd) => match &sprint_cmd.action {
                crate::cli::SprintAction::Create { board, .. }
                | crate::cli::SprintAction::List { board, .. } => {
                    scope.board = Some(Ref::of(board));
                }
                _ => {}
            },
            Commands::Relation(relation_cmd) => match &relation_cmd.action {
                RelationAction::Parents { .. } | RelationAction::Children { .. } => {
                    scope.wants_graph = true;
                }
                RelationAction::Add { .. } | RelationAction::Remove { .. } => {}
            },
            Commands::Export(args) => {
                scope.board = args.board.as_deref().map(Ref::of);
            }
            Commands::Import(_)
            | Commands::Completions { .. }
            | Commands::Migrate(_)
            | Commands::Init { .. } => {}
        }

        scope
    }
}

impl FetchPlan for CommandScope {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{
        BoardCommand, CardCommand, CardListArgs, ExportArgs, ImportArgs, RelationCommand, SortDir,
        SortKey,
    };
    use kanban_domain::{EntityIds, KanbanError, Model};
    use std::sync::Arc;

    #[test]
    fn test_command_scope_from_a_named_board_requests_the_board_list() {
        let cmd = Commands::Board(BoardCommand {
            action: BoardAction::Get {
                board: "Kanban".into(),
            },
        });
        let scope = CommandScope::from_command(&cmd);
        let round = scope.next_round(&Model::default());

        assert!(round.board_list);
        assert_eq!(
            round,
            FetchRound {
                board_list: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_command_scope_from_a_uuid_board_requests_nothing() {
        let cmd = Commands::Board(BoardCommand {
            action: BoardAction::Get {
                board: Uuid::new_v4().to_string(),
            },
        });
        let scope = CommandScope::from_command(&cmd);

        assert!(scope.next_round(&Model::default()).is_empty());
    }

    #[test]
    fn test_command_scope_for_export_with_a_named_board_requests_the_board_list() {
        let cmd = Commands::Export(ExportArgs {
            board: Some("Kanban".into()),
        });
        let scope = CommandScope::from_command(&cmd);

        assert!(scope.next_round(&Model::default()).board_list);
    }

    #[test]
    fn test_command_scope_for_import_requests_nothing() {
        let cmd = Commands::Import(ImportArgs { file: "x.json".into() });
        let scope = CommandScope::from_command(&cmd);

        assert!(scope.next_round(&Model::default()).is_empty());
    }

    #[test]
    fn test_command_scope_for_relation_children_requests_the_graph() {
        let cmd = Commands::Relation(RelationCommand {
            action: RelationAction::Children {
                card: "KAN-1".into(),
                sort: SortKey::CardNumber,
                order: SortDir::Asc,
            },
        });
        let scope = CommandScope::from_command(&cmd);
        let round = scope.next_round(&Model::default());

        assert!(round.graph);
        assert!(!round.board_list);

        let add_cmd = Commands::Relation(RelationCommand {
            action: RelationAction::Add {
                parent: "KAN-1".into(),
                children: vec!["KAN-2".into()],
            },
        });
        let add_scope = CommandScope::from_command(&add_cmd);
        assert!(add_scope.next_round(&Model::default()).is_empty());
    }

    #[test]
    fn test_command_scope_stops_requesting_once_the_board_list_is_loaded() {
        let cmd = Commands::Board(BoardCommand {
            action: BoardAction::Get {
                board: "Kanban".into(),
            },
        });
        let scope = CommandScope::from_command(&cmd);

        let mut model = Model::default();
        let _ = model.apply_resolved(kanban_domain::Resolved {
            boards: kanban_domain::resolved::Collection {
                all: kanban_domain::LoadState::Loaded(vec![]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(scope.next_round(&model).is_empty());
    }

    #[test]
    fn test_command_scope_retries_a_failed_board_list() {
        let cmd = Commands::Board(BoardCommand {
            action: BoardAction::Get {
                board: "Kanban".into(),
            },
        });
        let scope = CommandScope::from_command(&cmd);

        let mut model = Model::default();
        let _ = model.mark_failed(
            EntityIds::boards([Uuid::new_v4()]),
            Arc::new(KanbanError::Database("boom".into())),
        );

        assert!(scope.next_round(&model).board_list);
    }

    #[test]
    fn test_command_scope_does_not_request_a_missing_board_list() {
        let cmd = Commands::Board(BoardCommand {
            action: BoardAction::Get {
                board: "Kanban".into(),
            },
        });
        let scope = CommandScope::from_command(&cmd);

        let mut model = Model::default();
        let _ = model.apply_resolved(kanban_domain::Resolved {
            boards: kanban_domain::resolved::Collection {
                all: kanban_domain::LoadState::Missing,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(scope.next_round(&model).is_empty());
    }

    #[test]
    fn test_command_scope_for_card_list_without_a_board_requests_nothing() {
        let cmd = Commands::Card(CardCommand {
            action: crate::cli::CardAction::List(CardListArgs {
                board: None,
                column: None,
                sprint: None,
                status: None,
                archived: false,
                include_archived: false,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }),
        });
        let scope = CommandScope::from_command(&cmd);

        assert!(scope.next_round(&Model::default()).is_empty());
    }
}
