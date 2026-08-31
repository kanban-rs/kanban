#![allow(dead_code)]

use crate::helpers::error_mapping::kanban_err_to_mcp;
use kanban_domain::{KanbanError, LoadState, Model};
use rmcp::model::ErrorData as McpError;
use uuid::Uuid;

pub(crate) fn require_loaded<T>(state: LoadState<T>, what: &str) -> Result<T, McpError> {
    match state {
        LoadState::Loaded(value) => Ok(value),
        LoadState::NotLoaded => Err(kanban_err_to_mcp(KanbanError::Internal(format!(
            "{what} was not fetched for this tool call"
        )))),
        LoadState::Missing => Err(kanban_err_to_mcp(KanbanError::Internal(format!(
            "{what} is unavailable"
        )))),
        LoadState::Failed(e) => Err(kanban_err_to_mcp(KanbanError::Internal(format!(
            "{what}: {e}"
        )))),
    }
}

pub(crate) fn resolve_board(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    let _ = model;
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Board",
        raw,
        Vec::new(),
    )))
}

pub(crate) fn resolve_column_in_board(
    model: &Model,
    raw: &str,
    board_id: Uuid,
) -> Result<Uuid, McpError> {
    let _ = (model, board_id);
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Column",
        raw,
        Vec::new(),
    )))
}

pub(crate) fn resolve_column_global(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    let _ = model;
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Column",
        raw,
        Vec::new(),
    )))
}

pub(crate) fn resolve_sprint_in_board(
    model: &Model,
    raw: &str,
    board_id: Uuid,
) -> Result<Uuid, McpError> {
    let _ = (model, board_id);
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Sprint",
        raw,
        Vec::new(),
    )))
}

pub(crate) fn resolve_sprint_global(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    let _ = model;
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Sprint",
        raw,
        Vec::new(),
    )))
}

pub(crate) fn resolve_card(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    let _ = model;
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Card",
        raw,
        Vec::new(),
    )))
}

pub(crate) fn resolve_cards(model: &Model, raws: &[String]) -> Result<Vec<Uuid>, McpError> {
    let _ = model;
    Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
        "Card",
        raws.first().map(String::as_str).unwrap_or(""),
        Vec::new(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{resolved::Collection, Board, Card, Column, EntityIds, KanbanError, Resolved, Sprint};
    use std::sync::Arc;

    fn loaded_boards(boards: Vec<Board>) -> Model {
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Loaded(boards),
                ..Default::default()
            },
            ..Default::default()
        });
        model
    }

    #[test]
    fn test_a_not_loaded_board_list_errors_instead_of_reporting_not_found() {
        let err = resolve_board(&Model::default(), "Kanban").unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(!err.message.contains("not found"));
    }

    #[test]
    fn test_a_failed_board_list_errors_naming_the_collection() {
        let mut model = Model::default();
        let _ = model.mark_failed(
            EntityIds::boards([Uuid::new_v4()]),
            Arc::new(KanbanError::Database("boom".into())),
        );

        let err = resolve_board(&model, "Kanban").unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(err.message.contains("boom"));

        let not_loaded_err = resolve_board(&Model::default(), "Kanban").unwrap_err();
        assert_ne!(err.message, not_loaded_err.message);
    }

    #[test]
    fn test_resolve_board_by_name_from_the_model_returns_its_id() {
        let board = Board::new("Kanban", None::<String>);
        let board_id = board.id;
        let model = loaded_boards(vec![board]);

        assert_eq!(resolve_board(&model, "Kanban").unwrap(), board_id);

        let uuid_raw = Uuid::new_v4().to_string();
        assert_eq!(
            resolve_board(&Model::default(), &uuid_raw).unwrap(),
            Uuid::parse_str(&uuid_raw).unwrap()
        );

        assert!(resolve_board(&model, "Nope").is_err());

        let dup_model = loaded_boards(vec![
            Board::new("Kanban", None::<String>),
            Board::new("Kanban", None::<String>),
        ]);
        assert!(resolve_board(&dup_model, "Kanban").is_err());
    }

    #[test]
    fn test_resolve_column_in_board_reads_the_parent_scoped_tier() {
        let board_id = Uuid::new_v4();
        let column = Column::new(board_id, "TODO", 0);
        let column_id = column.id;

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![column]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            resolve_column_in_board(&model, "TODO", board_id).unwrap(),
            column_id
        );

        let err = resolve_column_in_board(&Model::default(), "TODO", board_id).unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(!err.message.contains("not found"));
    }

    #[test]
    fn test_resolve_column_global_reads_the_flat_tier() {
        let board_id = Uuid::new_v4();
        let column = Column::new(board_id, "TODO", 0);

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            columns: Collection {
                all: LoadState::Loaded(vec![column]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(resolve_column_global(&model, "TODO").is_ok());

        let err = resolve_column_global(&Model::default(), "TODO").unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("column list"));

        let board_a = Uuid::new_v4();
        let board_b = Uuid::new_v4();
        let mut dup_model = Model::default();
        let _ = dup_model.apply_resolved(Resolved {
            columns: Collection {
                all: LoadState::Loaded(vec![
                    Column::new(board_a, "TODO", 0),
                    Column::new(board_b, "TODO", 0),
                ]),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(resolve_column_global(&dup_model, "TODO").is_err());
    }

    #[test]
    fn test_resolve_sprint_in_board_reads_the_scoped_tier_and_the_board() {
        let board = Board::new("Kanban", None::<String>);
        let board_id = board.id;
        let sprint = Sprint::new(board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![board]),
                ..Default::default()
            },
            sprints: Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![sprint]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            resolve_sprint_in_board(&model, "1", board_id).unwrap(),
            sprint_id
        );

        let err =
            resolve_sprint_in_board(&Model::default(), "1", board_id).unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(!err.message.contains("not found"));
    }

    #[test]
    fn test_resolve_sprint_global_reads_the_flat_sprint_and_board_tiers() {
        let board = Board::new("Kanban", None::<String>);
        let board_id = board.id;
        let sprint = Sprint::new(board_id, 1, None, None::<String>);

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![board]),
                ..Default::default()
            },
            sprints: Collection {
                all: LoadState::Loaded(vec![sprint]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(resolve_sprint_global(&model, "1").is_ok());

        let err = resolve_sprint_global(&Model::default(), "1").unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn test_resolve_card_by_identifier_reads_the_card_list() {
        let mut card = Card::new(Uuid::new_v4(), Uuid::new_v4(), "Title", 0);
        card.prefix = "KAN".into();
        card.card_number = 5;
        let card_id = card.id;

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![card]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(resolve_card(&model, "KAN-5").unwrap(), card_id);
        assert_eq!(resolve_card(&model, "5").unwrap(), card_id);

        let err = resolve_card(&Model::default(), "KAN-5").unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("card list"));

        assert!(resolve_card(&model, "KAN-999").is_err());
    }

    #[test]
    fn test_resolve_cards_reports_per_input_failures_and_distinguishes_an_unloaded_tier() {
        let mut card = Card::new(Uuid::new_v4(), Uuid::new_v4(), "Title", 0);
        card.prefix = "KAN".into();
        card.card_number = 5;
        let card_id = card.id;

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![card]),
                ..Default::default()
            },
            ..Default::default()
        });

        let ids = resolve_cards(&model, &["KAN-5".to_string(), "KAN-999".to_string()]);
        assert!(ids.is_err());
        let err = ids.unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("KAN-999"));
        let _ = card_id;

        let unloaded_err = resolve_cards(
            &Model::default(),
            &["KAN-5".to_string(), "KAN-999".to_string()],
        )
        .unwrap_err();
        assert_eq!(unloaded_err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(unloaded_err.message.contains("card list"));
    }
}
