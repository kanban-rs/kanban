use crate::context::McpContext;
use crate::helpers::error_mapping::kanban_err_to_mcp;
use kanban_domain::{
    find_boards_by_name, find_columns_by_name, find_sprints_by_query_global,
    find_sprints_by_query_on_board, AmbiguousMatch, Board, KanbanError, KanbanOperations,
    LoadState, Model, Sprint,
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

const MAX_ENUMERATED: usize = 20;

fn capped(mut labels: Vec<String>) -> Vec<String> {
    if labels.len() > MAX_ENUMERATED {
        let extra = labels.len() - MAX_ENUMERATED;
        labels.truncate(MAX_ENUMERATED);
        labels.push(format!("... and {extra} more"));
    }
    labels
}

fn sprint_alternatives(sprints: &[Sprint], boards: &[Board]) -> Vec<String> {
    capped(
        sprints
            .iter()
            .map(|s| {
                let label = match boards.iter().find(|b| b.id == s.board_id) {
                    Some(b) => s.get_name(b).unwrap_or("(unnamed)").to_string(),
                    None => "(unknown board)".to_string(),
                };
                format!("#{} {}", s.sprint_number, label)
            })
            .collect(),
    )
}

pub(crate) fn resolve_column_global(ctx: &McpContext, raw: &str) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let columns = ctx.list_all_columns().map_err(kanban_err_to_mcp)?;
    let matches = find_columns_by_name(raw, &columns);
    match matches.as_slice() {
        [] => Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
            "Column",
            raw,
            capped(columns.iter().map(|c| c.name.clone()).collect()),
        ))),
        [c] => Ok(c.id),
        many => {
            let boards = ctx.list_boards().map_err(kanban_err_to_mcp)?;
            let matches: Vec<AmbiguousMatch> = many
                .iter()
                .map(|c| {
                    let board_name = boards
                        .iter()
                        .find(|b| b.id == c.board_id)
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
    board: &Board,
) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let sprints = require_loaded(model.board_sprints_state(board.id), "sprints of the board")?;
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

pub(crate) fn resolve_sprint_global(ctx: &McpContext, raw: &str) -> Result<Uuid, McpError> {
    resolve_sprint_global_with_boards(ctx, raw).map(|(id, _)| id)
}

/// The resolved sprint id plus the board list the name-based lookup had to
/// fetch; `None` when `raw` parsed as a UUID and no board list was read.
pub(crate) fn resolve_sprint_global_with_boards(
    ctx: &McpContext,
    raw: &str,
) -> Result<(Uuid, Option<Vec<Board>>), McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok((uuid, None));
    }
    let all_sprints = ctx.list_all_sprints().map_err(kanban_err_to_mcp)?;
    let boards = ctx.list_boards().map_err(kanban_err_to_mcp)?;
    let matches = find_sprints_by_query_global(raw, &all_sprints, &boards);
    match matches.as_slice() {
        [] => {
            let available = sprint_alternatives(&all_sprints, &boards);
            Err(kanban_err_to_mcp(KanbanError::not_found_by_name(
                "Sprint", raw, available,
            )))
        }
        [s] => Ok((s.id, Some(boards))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{
        resolved::Collection, Board, Column, EntityIds, KanbanError, Resolved, Sprint,
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
    fn test_resolve_sprint_in_board_reads_the_scoped_tier_with_the_supplied_head() {
        let board = Board::new("Kanban", None::<String>);
        let board_id = board.id;
        let sprint = Sprint::new(board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            sprints: Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![sprint]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            resolve_sprint_in_board(&model, "1", &board).unwrap(),
            sprint_id
        );

        let err = resolve_sprint_in_board(&Model::default(), "1", &board).unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(!err.message.contains("not found"));
    }

    fn test_store_manager() -> kanban_service::StoreManager {
        let mut registry = kanban_persistence::StoreRegistry::new();
        let mut backends = kanban_backend::KanbanBackendRegistry::new();
        backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
        registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
        backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
        kanban_service::StoreManager::new(registry, backends)
    }

    #[tokio::test]
    async fn test_global_sprint_miss_caps_the_enumerated_alternatives() {
        use kanban_core::AppConfig;
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.json");
        let store_manager = test_store_manager();
        let mut ctx = McpContext::new(
            &store_manager,
            &path.to_string_lossy(),
            AppConfig::default(),
        )
        .await
        .unwrap();

        let board = ctx.create_board("Board".into(), None).unwrap();
        for _ in 0..25 {
            ctx.create_sprint(board.id, None, None).unwrap();
        }

        let err = resolve_sprint_global(&ctx, "no-such-sprint").unwrap_err();
        assert!(err.message.contains("and 5 more"));
        assert!(!err.message.contains("#25"));
    }

    #[tokio::test]
    async fn test_global_column_miss_caps_the_enumerated_alternatives() {
        use kanban_core::AppConfig;
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.json");
        let store_manager = test_store_manager();
        let mut ctx = McpContext::new(
            &store_manager,
            &path.to_string_lossy(),
            AppConfig::default(),
        )
        .await
        .unwrap();

        let board = ctx.create_board("Board".into(), None).unwrap();
        for i in 0..25 {
            ctx.create_column(board.id, format!("C{i}"), None).unwrap();
        }

        let err = resolve_column_global(&ctx, "no-such-column").unwrap_err();
        assert!(err.message.contains("and 5 more"));
        assert!(!err.message.contains("C24"));
    }

    #[test]
    fn test_sprint_alternatives_labels_a_sprint_whose_board_is_absent_as_unknown_board() {
        let sprint = Sprint::new(Uuid::new_v4(), 1, None, None::<String>);
        let labels = sprint_alternatives(&[sprint], &[]);
        assert_eq!(labels, vec!["#1 (unknown board)".to_string()]);

        let board = Board::new("Board", None::<String>);
        let sprint_on_board = Sprint::new(board.id, 1, None, None::<String>);
        let labels = sprint_alternatives(&[sprint_on_board], &[board]);
        assert_eq!(labels, vec!["#1 (unnamed)".to_string()]);
    }
}
