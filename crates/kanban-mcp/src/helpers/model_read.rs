use crate::helpers::error_mapping::kanban_err_to_mcp;
use kanban_domain::{
    find_boards_by_name, find_columns_by_name, find_sprints_by_query_global,
    find_sprints_by_query_on_board, parse_identifier, AmbiguousMatch, BatchResolutionCause,
    BatchResolutionFailure, Card, KanbanError, LoadState, Model, ParsedIdentifier, Prefix,
};
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
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let boards = require_loaded(model.boards_state().as_ref(), "board list")?;
    let matches = find_boards_by_name(raw, boards);
    match matches.as_slice() {
        [] => Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
            "Board",
            raw,
            boards.iter().map(|b| b.name.clone()).collect(),
        ))),
        [b] => Ok(b.id),
        many => Err(kanban_err_to_mcp(KanbanError::ambiguous(
            "Board",
            raw,
            many.iter()
                .map(|b| AmbiguousMatch {
                    label: format!("'{}'", b.name),
                    id: b.id,
                })
                .collect(),
        ))),
    }
}

pub(crate) fn resolve_column_in_board(
    model: &Model,
    raw: &str,
    board_id: Uuid,
) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let columns = require_loaded(model.board_columns_state(board_id), "columns of the board")?;
    let matches = find_columns_by_name(raw, columns);
    match matches.as_slice() {
        [] => Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
            "Column",
            raw,
            columns.iter().map(|c| c.name.clone()).collect(),
        ))),
        [c] => Ok(c.id),
        many => Err(kanban_err_to_mcp(KanbanError::ambiguous(
            "Column",
            raw,
            many.iter()
                .map(|c| AmbiguousMatch {
                    label: format!("'{}'", c.name),
                    id: c.id,
                })
                .collect(),
        ))),
    }
}

pub(crate) fn resolve_column_global(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let columns = require_loaded(model.columns_state().as_ref(), "column list")?;
    let matches = find_columns_by_name(raw, columns);
    match matches.as_slice() {
        [] => Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
            "Column",
            raw,
            columns.iter().map(|c| c.name.clone()).collect(),
        ))),
        [c] => Ok(c.id),
        many => {
            let boards = model.boards_state().loaded();
            let matches: Vec<AmbiguousMatch> = many
                .iter()
                .map(|c| {
                    let board_name = boards
                        .and_then(|bs| bs.iter().find(|b| b.id == c.board_id))
                        .map(|b| b.name.as_str())
                        .unwrap_or("(unknown)");
                    AmbiguousMatch {
                        label: format!("on board '{}'", board_name),
                        id: c.id,
                    }
                })
                .collect();
            Err(kanban_err_to_mcp(KanbanError::ambiguous(
                "Column", raw, matches,
            )))
        }
    }
}

pub(crate) fn resolve_sprint_in_board(
    model: &Model,
    raw: &str,
    board_id: Uuid,
) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let sprints = require_loaded(model.board_sprints_state(board_id), "sprints of the board")?;
    let board = require_loaded(model.board_by_id_state(board_id), "board")?;
    let matches = find_sprints_by_query_on_board(raw, sprints, board);
    match matches.as_slice() {
        [] => {
            let available = sprints
                .iter()
                .map(|s| {
                    let label = s.get_name(board).unwrap_or("(unnamed)");
                    format!("#{} {}", s.sprint_number, label)
                })
                .collect();
            Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
                "Sprint", raw, available,
            )))
        }
        [s] => Ok(s.id),
        many => Err(kanban_err_to_mcp(KanbanError::ambiguous(
            "Sprint",
            raw,
            many.iter()
                .map(|s| {
                    let name = s.get_name(board).unwrap_or("(unnamed)");
                    AmbiguousMatch {
                        label: format!("#{} '{}'", s.sprint_number, name),
                        id: s.id,
                    }
                })
                .collect(),
        ))),
    }
}

pub(crate) fn resolve_sprint_global(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let all_sprints = require_loaded(model.sprints_state().as_ref(), "sprint list")?;
    let boards = require_loaded(model.boards_state().as_ref(), "board list")?;
    let matches = find_sprints_by_query_global(raw, all_sprints, boards);
    match matches.as_slice() {
        [] => {
            let available = all_sprints
                .iter()
                .map(|s| {
                    let label = boards
                        .iter()
                        .find(|b| b.id == s.board_id)
                        .and_then(|b| s.get_name(b))
                        .unwrap_or("(unnamed)");
                    format!("#{} {}", s.sprint_number, label)
                })
                .collect();
            Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
                "Sprint", raw, available,
            )))
        }
        [s] => Ok(s.id),
        many => {
            let matches: Vec<AmbiguousMatch> = many
                .iter()
                .map(|s| {
                    let board = boards.iter().find(|b| b.id == s.board_id);
                    let board_name = board.map(|b| b.name.as_str()).unwrap_or("(unknown)");
                    let sprint_name = board.and_then(|b| s.get_name(b)).unwrap_or("(unnamed)");
                    AmbiguousMatch {
                        label: format!(
                            "#{} '{}' on board '{}'",
                            s.sprint_number, sprint_name, board_name
                        ),
                        id: s.id,
                    }
                })
                .collect();
            Err(kanban_err_to_mcp(KanbanError::ambiguous(
                "Sprint", raw, matches,
            )))
        }
    }
}

fn find_card_matches<'a>(
    cards: &'a [kanban_domain::Card],
    raw: &str,
) -> Vec<&'a kanban_domain::Card> {
    match parse_identifier(raw) {
        Some(ParsedIdentifier::PrefixAndNumber { prefix, number }) => cards
            .iter()
            .filter(|c| c.card_number == number && Prefix::normalize(&c.prefix) == prefix)
            .collect(),
        Some(ParsedIdentifier::NumberOnly(number)) => {
            cards.iter().filter(|c| c.card_number == number).collect()
        }
        None => Vec::new(),
    }
}

pub(crate) fn resolve_card(model: &Model, raw: &str) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let cards = require_loaded(model.cards_state().as_ref(), "card list")?;
    let matches = find_card_matches(cards, raw);
    match matches.as_slice() {
        [] => Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
            "Card",
            raw,
            Vec::new(),
        ))),
        [c] => Ok(c.id),
        many => Err(kanban_err_to_mcp(KanbanError::ambiguous(
            "Card",
            raw,
            many.iter()
                .map(|c| AmbiguousMatch {
                    label: format!("'{}'", c.title),
                    id: c.id,
                })
                .collect(),
        ))),
    }
}

pub(crate) fn resolve_cards(model: &Model, raws: &[String]) -> Result<Vec<Uuid>, McpError> {
    let mut resolved = Vec::with_capacity(raws.len());
    let mut failures = Vec::new();
    let mut cards: Option<&Vec<Card>> = None;
    for raw in raws {
        if let Ok(uuid) = Uuid::parse_str(raw) {
            resolved.push(uuid);
            continue;
        }
        let cards = match cards {
            Some(cards) => cards,
            None => {
                let loaded = require_loaded(model.cards_state().as_ref(), "card list")?;
                cards = Some(loaded);
                loaded
            }
        };
        let matches = find_card_matches(cards, raw);
        match matches.as_slice() {
            [] => failures.push(BatchResolutionFailure {
                raw_input: raw.clone(),
                cause: BatchResolutionCause::NotFound,
            }),
            [c] => resolved.push(c.id),
            many => failures.push(BatchResolutionFailure {
                raw_input: raw.clone(),
                cause: BatchResolutionCause::Ambiguous(
                    many.iter()
                        .map(|c| AmbiguousMatch {
                            label: format!("'{}'", c.title),
                            id: c.id,
                        })
                        .collect(),
                ),
            }),
        }
    }
    if !failures.is_empty() {
        return Err(kanban_err_to_mcp(KanbanError::batch_resolution_failed(
            "Card", failures,
        )));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{
        resolved::Collection, Board, Card, Column, EntityIds, KanbanError, Resolved, Sprint,
    };
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

        let err = resolve_sprint_in_board(&Model::default(), "1", board_id).unwrap_err();
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

    #[test]
    fn test_resolve_cards_with_only_uuid_references_does_not_require_the_card_list() {
        let a = Uuid::new_v4().to_string();
        let b = Uuid::new_v4().to_string();
        let ids = resolve_cards(&Model::default(), &[a.clone(), b.clone()]).unwrap();
        assert_eq!(
            ids,
            vec![Uuid::parse_str(&a).unwrap(), Uuid::parse_str(&b).unwrap()]
        );

        let mixed_err = resolve_cards(&Model::default(), &[a, "KAN-5".to_string()]).unwrap_err();
        assert_eq!(mixed_err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(mixed_err.message.contains("card list"));
    }
}
