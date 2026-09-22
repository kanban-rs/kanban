use crate::helpers::model_read::{
    resolve_board, resolve_sprint_in_board, resolve_sprint_with_optional_board,
};
use crate::helpers::{
    board_head, core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, parse_datetime,
    project_sprint, to_call_tool_result, to_call_tool_result_json,
};
use crate::requests::sprint::{
    ActivateSprintRequest, CancelSprintRequest, CarryOverSprintCardsRequest, CompleteSprintRequest,
    CreateSprintParams, DeleteSprintRequest, GetSprintRequest, ListSprintsRequest,
    UpdateSprintRequest,
};
use crate::scope::{Ref, ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{FieldUpdate, KanbanOperations, Model, SprintUpdate};
use kanban_service::api::SprintResponse;
use kanban_service::{resolve_sprint_name, resolve_sprint_names};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

impl ToolScoped for CreateSprintParams {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for ListSprintsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for GetSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for UpdateSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for ActivateSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for CompleteSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for CancelSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for DeleteSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for CarryOverSprintCardsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: self.board.as_deref().map(Ref::of),
            wants_board_sprints: true,
            ..Default::default()
        }
    }
}

#[tool_router(router = sprint_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new sprint")]
    pub async fn tool_create_sprint(
        &self,
        Parameters(req): Parameters<CreateSprintParams>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            // Resolve the parent board (name→id), then funnel the shared DTO
            // content through the Sprint factory via `create_sprint_from_spec`.
            // The JSON edge projects the domain Sprint via SprintResponse,
            // resolving the sprint name against its owning board.
            let model = ctx.model_for(&scope);
            let board_id = resolve_board(&model, &req.board)?;
            let content = req.content;
            let (sprint, _inv) = ctx
                .mutate(|c| {
                    c.create_sprint_from_spec(
                        board_id,
                        content.id,
                        content.name,
                        content.prefix,
                        false,
                    )
                })
                .map_err(kanban_err_to_mcp)?;
            let name = resolve_sprint_name(ctx, &sprint).map_err(kanban_err_to_mcp)?;
            Ok(SprintResponse::new(&sprint, name))
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(
        description = "List sprints for a board. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_sprints(
        &self,
        Parameters(req): Parameters<ListSprintsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let responses = locked_read(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let board_id = resolve_board(&model, &req.board)?;
            let sprints = ctx.list_sprints(board_id).map_err(kanban_err_to_mcp)?;
            let names = resolve_sprint_names(ctx, board_id, &sprints).map_err(kanban_err_to_mcp)?;
            Ok(sprints
                .iter()
                .zip(names)
                .map(|(s, name)| SprintResponse::new(s, name))
                .collect::<Vec<_>>())
        })
        .await?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(responses, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(description = "Get a specific sprint by UUID, name, or number")]
    pub async fn tool_get_sprint(
        &self,
        Parameters(req): Parameters<GetSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let response = locked_read(&self.ctx, |ctx| -> Result<_, McpError> {
            let id =
                resolve_sprint_with_optional_board(ctx, &req.sprint, req.board.as_deref(), scope)?;
            let Some(sprint) = ctx.get_sprint(id).map_err(kanban_err_to_mcp)? else {
                return Ok(None);
            };
            let name = resolve_sprint_name(ctx, &sprint).map_err(kanban_err_to_mcp)?;
            Ok(Some(SprintResponse::new(&sprint, name)))
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(
        description = "Update a sprint's properties (name, prefix, card_prefix, start_date, end_date)"
    )]
    pub async fn tool_update_sprint(
        &self,
        Parameters(req): Parameters<UpdateSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let start_date = if req.clear_start_date == Some(true) {
            FieldUpdate::Clear
        } else {
            match req.start_date {
                Some(ref d) => FieldUpdate::Set(parse_datetime(d)?),
                None => FieldUpdate::NoChange,
            }
        };
        let end_date = if req.clear_end_date == Some(true) {
            FieldUpdate::Clear
        } else {
            match req.end_date {
                Some(ref d) => FieldUpdate::Set(parse_datetime(d)?),
                None => FieldUpdate::NoChange,
            }
        };
        let updates = SprintUpdate {
            name: req.name.clone(),
            name_index: FieldUpdate::NoChange,
            prefix: req
                .prefix
                .clone()
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            card_prefix: req
                .card_prefix
                .clone()
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            status: None,
            start_date,
            end_date,
        };
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id =
                resolve_sprint_with_optional_board(ctx, &req.sprint, req.board.as_deref(), scope)?;
            let (sprint, _inv) = ctx
                .mutate(|c| c.update_sprint_impl(id, updates))
                .map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Activate a sprint")]
    pub async fn tool_activate_sprint(
        &self,
        Parameters(req): Parameters<ActivateSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id =
                resolve_sprint_with_optional_board(ctx, &req.sprint, req.board.as_deref(), scope)?;
            let (sprint, _inv) = ctx
                .mutate(|c| c.activate_sprint_impl(id, req.duration_days))
                .map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Complete a sprint")]
    pub async fn tool_complete_sprint(
        &self,
        Parameters(req): Parameters<CompleteSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id =
                resolve_sprint_with_optional_board(ctx, &req.sprint, req.board.as_deref(), scope)?;
            let (sprint, _inv) = ctx
                .mutate(|c| c.complete_sprint_impl(id))
                .map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Cancel a sprint")]
    pub async fn tool_cancel_sprint(
        &self,
        Parameters(req): Parameters<CancelSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id =
                resolve_sprint_with_optional_board(ctx, &req.sprint, req.board.as_deref(), scope)?;
            let (sprint, _inv) = ctx
                .mutate(|c| c.cancel_sprint_impl(id))
                .map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Delete a sprint")]
    pub async fn tool_delete_sprint(
        &self,
        Parameters(req): Parameters<DeleteSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id =
                resolve_sprint_with_optional_board(ctx, &req.sprint, req.board.as_deref(), scope)?;
            let _inv = ctx
                .mutate_unit(|c| c.delete_sprint_impl(id))
                .map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }

    #[tool(
        description = "Carry over uncompleted cards from a completed/cancelled sprint to a planning sprint on the same board"
    )]
    pub async fn tool_carry_over_sprint_cards(
        &self,
        Parameters(req): Parameters<CarryOverSprintCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let count = locked_write(&self.ctx, |ctx| {
            let from_id = resolve_sprint_with_optional_board(
                ctx,
                &req.from_sprint,
                req.board.as_deref(),
                scope,
            )?;
            let from_sprint = ctx
                .get_sprint(from_id)
                .map_err(kanban_err_to_mcp)?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Sprint not found: {}", from_id), None)
                })?;
            let to_scope = ToolScope {
                wants_board_sprints: true,
                ..Default::default()
            }
            .for_board(from_sprint.board_id);
            let mut model = Model::default();
            ctx.sync_into(&to_scope, &mut model);
            let board = board_head(ctx, &model, from_sprint.board_id)?;
            let to_id = resolve_sprint_in_board(&model, &req.to_sprint, &board)?;
            ctx.mutate(|c| c.carry_over_sprint_cards_impl(from_id, to_id))
                .map(|(count, _inv)| count)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({ "carried_over_count": count }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::board::CreateBoardRequest;
    use crate::requests::transfer::ExportBoardRequest;
    use crate::scope::ToolScoped;
    use crate::McpServer;
    use kanban_backend::{KanbanBackend, KanbanBackendFactory};
    use kanban_core::AppConfig;
    use kanban_domain::Model;
    use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};
    use kanban_persistence_sqlite::SqliteBackendFactory;
    use kanban_service::test_helpers::FaultInjectingBackend;
    use kanban_service::FetchPlan;
    use rmcp::model::ErrorCode;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_sprint_request_scopes_plan_the_board_list_and_the_boards_sprints_for_a_named_sprint() {
        let board_id = Uuid::new_v4();

        let get_no_board = GetSprintRequest {
            board: None,
            sprint: "Sprint 1".into(),
        };
        assert!(get_no_board
            .scope()
            .next_round(&Model::default())
            .is_empty());

        let get_named_board = GetSprintRequest {
            board: Some("Alpha".into()),
            sprint: "Sprint 1".into(),
        };
        assert!(get_named_board.scope().wants_board_sprints);
        assert!(
            get_named_board
                .scope()
                .next_round(&Model::default())
                .board_list
        );
        assert_eq!(
            get_named_board
                .scope()
                .for_board(board_id)
                .next_round(&Model::default())
                .sprints_by_board,
            vec![board_id]
        );

        let get_id_board = GetSprintRequest {
            board: Some(Uuid::new_v4().to_string()),
            sprint: "Sprint 1".into(),
        };
        assert!(
            !get_id_board
                .scope()
                .next_round(&Model::default())
                .board_list
        );
        assert_eq!(
            get_id_board
                .scope()
                .for_board(board_id)
                .next_round(&Model::default())
                .sprints_by_board,
            vec![board_id]
        );

        let id_get = GetSprintRequest {
            board: None,
            sprint: Uuid::new_v4().to_string(),
        };
        assert!(id_get.scope().next_round(&Model::default()).is_empty());

        let update = UpdateSprintRequest {
            board: Some("Alpha".into()),
            sprint: "Sprint 1".into(),
            name: None,
            prefix: None,
            card_prefix: None,
            start_date: None,
            end_date: None,
            clear_start_date: None,
            clear_end_date: None,
        };
        assert!(update.scope().next_round(&Model::default()).board_list);

        let activate = ActivateSprintRequest {
            board: Some("Alpha".into()),
            sprint: "Sprint 1".into(),
            duration_days: None,
        };
        assert!(activate.scope().next_round(&Model::default()).board_list);

        let complete = CompleteSprintRequest {
            board: Some("Alpha".into()),
            sprint: "Sprint 1".into(),
        };
        assert!(complete.scope().next_round(&Model::default()).board_list);

        let cancel = CancelSprintRequest {
            board: Some("Alpha".into()),
            sprint: "Sprint 1".into(),
        };
        assert!(cancel.scope().next_round(&Model::default()).board_list);

        let delete = DeleteSprintRequest {
            board: Some("Alpha".into()),
            sprint: "Sprint 1".into(),
        };
        assert!(delete.scope().next_round(&Model::default()).board_list);

        let carry_over = CarryOverSprintCardsRequest {
            board: Some("Alpha".into()),
            from_sprint: "Sprint 1".into(),
            to_sprint: "Sprint 2".into(),
        };
        assert!(carry_over.scope().next_round(&Model::default()).board_list);

        let name_board = CreateSprintParams {
            board: "Alpha".into(),
            content: kanban_service::api::CreateSprintRequest {
                id: None,
                name: None,
                prefix: None,
                card_prefix: None,
            },
        };
        assert!(name_board.scope().next_round(&Model::default()).board_list);

        let id_board = ListSprintsRequest {
            board: Uuid::new_v4().to_string(),
            page: None,
            page_size: None,
        };
        assert!(id_board.scope().next_round(&Model::default()).is_empty());
    }

    struct RecordingFactory {
        inner: Box<dyn KanbanBackendFactory>,
        handle: Arc<Mutex<Option<Arc<FaultInjectingBackend>>>>,
    }

    #[async_trait::async_trait]
    impl KanbanBackendFactory for RecordingFactory {
        fn name(&self) -> &str {
            self.inner.name()
        }

        fn matches_locator(&self, locator: &str, header: &[u8]) -> bool {
            self.inner.matches_locator(locator, header)
        }

        async fn create(
            &self,
            locator: &str,
            config: &AppConfig,
        ) -> kanban_domain::KanbanResult<Arc<dyn KanbanBackend>> {
            let inner = self.inner.create(locator, config).await?;
            let wrapped = Arc::new(FaultInjectingBackend::new(inner));
            *self.handle.lock().unwrap() = Some(Arc::clone(&wrapped));
            Ok(wrapped as Arc<dyn KanbanBackend>)
        }
    }

    fn text_payload(result: &rmcp::model::CallToolResult) -> serde_json::Value {
        let raw = &result.content[0]
            .as_text()
            .expect("expected text content")
            .text;
        serde_json::from_str(raw).expect("tool result is JSON")
    }

    struct Seeded {
        server: KanbanMcpServer,
        _dir: TempDir,
        handle: Arc<FaultInjectingBackend>,
    }

    async fn seeded_server(file_name: &str) -> Seeded {
        let sqlite_handle = Arc::new(Mutex::new(None));
        let json_handle = Arc::new(Mutex::new(None));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(file_name);

        let server = McpServer::default()
            .register_backend_only(Box::new(RecordingFactory {
                inner: Box::new(SqliteBackendFactory),
                handle: Arc::clone(&sqlite_handle),
            }))
            .register_backend(
                Box::new(JsonStoreFactory),
                Box::new(RecordingFactory {
                    inner: Box::new(JsonBackendFactory),
                    handle: Arc::clone(&json_handle),
                }),
            )
            .with_data_file(path.to_string_lossy().to_string())
            .build()
            .await
            .unwrap();

        server
            .tool_create_board(Parameters(crate::requests::board::CreateBoardParams {
                content: CreateBoardRequest {
                    id: None,
                    name: "Alpha".to_string(),
                    description: None,
                    sprint_prefix: None,
                    card_prefix: None,
                    task_sort_field: None,
                    task_sort_order: None,
                    sprint_duration_days: None,
                    task_list_view: None,
                },
                with_default_columns: None,
            }))
            .await
            .unwrap();

        let from_sprint = text_payload(
            &server
                .tool_create_sprint(Parameters(CreateSprintParams {
                    board: "Alpha".into(),
                    content: kanban_service::api::CreateSprintRequest {
                        id: None,
                        name: Some("From".into()),
                        prefix: None,
                        card_prefix: None,
                    },
                }))
                .await
                .unwrap(),
        );
        server
            .tool_create_sprint(Parameters(CreateSprintParams {
                board: "Alpha".into(),
                content: kanban_service::api::CreateSprintRequest {
                    id: None,
                    name: Some("To".into()),
                    prefix: None,
                    card_prefix: None,
                },
            }))
            .await
            .unwrap();

        let from_sprint_id = from_sprint["id"].as_str().unwrap().to_string();
        server
            .tool_activate_sprint(Parameters(ActivateSprintRequest {
                board: None,
                sprint: from_sprint_id.clone(),
                duration_days: None,
            }))
            .await
            .unwrap();
        server
            .tool_complete_sprint(Parameters(CompleteSprintRequest {
                board: None,
                sprint: from_sprint_id.clone(),
            }))
            .await
            .unwrap();

        let handle = sqlite_handle
            .lock()
            .unwrap()
            .clone()
            .or_else(|| json_handle.lock().unwrap().clone())
            .expect("a backend must have been created");
        Seeded {
            server,
            _dir: dir,
            handle,
        }
    }

    #[tokio::test]
    async fn test_carry_over_sprint_cards_resolves_the_to_sprint_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
                board: Some("Alpha".into()),
                from_sprint: "From".into(),
                to_sprint: "To".into(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("list_all_sprints"), 0);
        // Once for `from_sprint` and once for `to_sprint`: each resolves
        // through its own fresh model now that no board list is threaded
        // between them.
        assert_eq!(seeded.handle.op_count("list_sprints_by_board"), 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_carry_over_sprint_cards_resolves_the_to_sprint_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
                board: Some("Alpha".into()),
                from_sprint: "From".into(),
                to_sprint: "To".into(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("list_all_sprints"), 0);
        assert_eq!(seeded.handle.op_count("list_sprints_by_board"), 2);
    }

    #[tokio::test]
    async fn test_get_sprint_by_name_with_a_board_and_an_unloadable_sprint_list_errors_naming_the_collection_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                board: Some("Alpha".into()),
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err
            .message
            .contains("injected fault: list_sprints_by_board"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_sprint_by_name_with_a_board_and_an_unloadable_sprint_list_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                board: Some("Alpha".into()),
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err
            .message
            .contains("injected fault: list_sprints_by_board"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_every_sprint_tool_succeeds_end_to_end_by_name() {
        let seeded = seeded_server("test.json").await;

        let listed = seeded
            .server
            .tool_list_sprints(Parameters(ListSprintsRequest {
                board: "Alpha".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap();
        assert!(!text_payload(&listed)["items"]
            .as_array()
            .unwrap()
            .is_empty());

        seeded
            .server
            .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
                board: Some("Alpha".into()),
                from_sprint: "From".into(),
                to_sprint: "To".into(),
            }))
            .await
            .unwrap();

        seeded
            .server
            .tool_activate_sprint(Parameters(ActivateSprintRequest {
                board: Some("Alpha".into()),
                sprint: "To".into(),
                duration_days: None,
            }))
            .await
            .unwrap();

        seeded
            .server
            .tool_complete_sprint(Parameters(CompleteSprintRequest {
                board: Some("Alpha".into()),
                sprint: "To".into(),
            }))
            .await
            .unwrap();

        let updated = text_payload(
            &seeded
                .server
                .tool_update_sprint(Parameters(UpdateSprintRequest {
                    board: Some("Alpha".into()),
                    sprint: "To".into(),
                    name: Some("Renamed".into()),
                    prefix: None,
                    card_prefix: None,
                    start_date: None,
                    end_date: None,
                    clear_start_date: None,
                    clear_end_date: None,
                }))
                .await
                .unwrap(),
        );
        assert_eq!(updated["name"], "Renamed");

        seeded
            .server
            .tool_cancel_sprint(Parameters(CancelSprintRequest {
                board: Some("Alpha".into()),
                sprint: "Renamed".into(),
            }))
            .await
            .unwrap();

        let deleted = text_payload(
            &seeded
                .server
                .tool_delete_sprint(Parameters(DeleteSprintRequest {
                    board: Some("Alpha".into()),
                    sprint: "Renamed".into(),
                }))
                .await
                .unwrap(),
        );
        assert!(deleted["deleted"].is_string());

        let exported = seeded
            .server
            .tool_export_board(Parameters(ExportBoardRequest {
                board: Some("Alpha".into()),
            }))
            .await
            .unwrap();
        assert!(exported.content[0].as_text().is_some());
    }

    #[tokio::test]
    async fn test_get_sprint_by_number_with_a_board_does_not_match_a_same_numbered_sprint_on_another_board_on_json(
    ) {
        test_get_sprint_by_number_with_a_board_does_not_match_a_same_numbered_sprint_on_another_board(
            "test.json",
        )
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_sprint_by_number_with_a_board_does_not_match_a_same_numbered_sprint_on_another_board_on_sqlite(
    ) {
        test_get_sprint_by_number_with_a_board_does_not_match_a_same_numbered_sprint_on_another_board(
            "test.sqlite",
        )
        .await;
    }

    async fn test_get_sprint_by_number_with_a_board_does_not_match_a_same_numbered_sprint_on_another_board(
        file_name: &str,
    ) {
        let seeded = seeded_server(file_name).await;

        seeded
            .server
            .tool_create_board(Parameters(crate::requests::board::CreateBoardParams {
                content: CreateBoardRequest {
                    id: None,
                    name: "Beta".to_string(),
                    description: None,
                    sprint_prefix: Some("ZED".into()),
                    card_prefix: None,
                    task_sort_field: None,
                    task_sort_order: None,
                    sprint_duration_days: None,
                    task_list_view: None,
                },
                with_default_columns: None,
            }))
            .await
            .unwrap();

        let beta_sprint = text_payload(
            &seeded
                .server
                .tool_create_sprint(Parameters(CreateSprintParams {
                    board: "Beta".into(),
                    content: kanban_service::api::CreateSprintRequest {
                        id: None,
                        name: Some("Collides".into()),
                        prefix: None,
                        card_prefix: None,
                    },
                }))
                .await
                .unwrap(),
        );
        assert_eq!(beta_sprint["sprint_number"], 1);

        let result = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    board: Some("Alpha".into()),
                    sprint: "1".into(),
                }))
                .await
                .unwrap(),
        );
        assert_eq!(result["name"], "From");
    }

    #[tokio::test]
    async fn test_get_sprint_result_json_is_unchanged() {
        let seeded = seeded_server("test.json").await;

        let response = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    board: Some("Alpha".into()),
                    sprint: "From".into(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["name"], "From");
        assert!(response["id"].is_string());
        assert!(response["sprint_number"].is_number());
    }

    #[tokio::test]
    async fn test_get_sprint_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                board: None,
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_get_sprint_by_number_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                board: None,
                sprint: "1".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_update_sprint_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_update_sprint(Parameters(UpdateSprintRequest {
                board: None,
                sprint: "From".into(),
                name: None,
                prefix: None,
                card_prefix: None,
                start_date: None,
                end_date: None,
                clear_start_date: None,
                clear_end_date: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_activate_sprint_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_activate_sprint(Parameters(ActivateSprintRequest {
                board: None,
                sprint: "From".into(),
                duration_days: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_complete_sprint_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_complete_sprint(Parameters(CompleteSprintRequest {
                board: None,
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_cancel_sprint_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_cancel_sprint(Parameters(CancelSprintRequest {
                board: None,
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_delete_sprint_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_delete_sprint(Parameters(DeleteSprintRequest {
                board: None,
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_carry_over_sprint_cards_by_name_without_a_board_returns_a_validation_error() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
                board: None,
                from_sprint: "From".into(),
                to_sprint: "To".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("board"));
    }

    #[tokio::test]
    async fn test_board_less_sprint_name_error_names_both_remedies() {
        let seeded = seeded_server("test.json").await;

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                board: None,
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert!(err.message.contains("board"));
        assert!(err.message.contains("UUID"));
    }

    #[tokio::test]
    async fn test_get_sprint_by_uuid_without_a_board_still_resolves() {
        let seeded = seeded_server("test.json").await;
        let from = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    board: Some("Alpha".into()),
                    sprint: "From".into(),
                }))
                .await
                .unwrap(),
        );
        let from_id = from["id"].as_str().unwrap().to_string();

        let response = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    board: None,
                    sprint: from_id.clone(),
                }))
                .await
                .unwrap(),
        );
        assert_eq!(response["id"], from_id);
    }

    #[tokio::test]
    async fn test_get_sprint_by_name_with_a_board_resolves_within_that_board() {
        let seeded = seeded_server("test.json").await;

        let response = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    board: Some("Alpha".into()),
                    sprint: "From".into(),
                }))
                .await
                .unwrap(),
        );
        assert_eq!(response["name"], "From");
    }

    #[tokio::test]
    async fn test_get_sprint_by_name_with_a_board_does_not_match_a_same_named_sprint_on_another_board(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_create_board(Parameters(crate::requests::board::CreateBoardParams {
                content: CreateBoardRequest {
                    id: None,
                    name: "Beta".to_string(),
                    description: None,
                    sprint_prefix: Some("ZED".into()),
                    card_prefix: None,
                    task_sort_field: None,
                    task_sort_order: None,
                    sprint_duration_days: None,
                    task_list_view: None,
                },
                with_default_columns: None,
            }))
            .await
            .unwrap();
        let beta_sprint = text_payload(
            &seeded
                .server
                .tool_create_sprint(Parameters(CreateSprintParams {
                    board: "Beta".into(),
                    content: kanban_service::api::CreateSprintRequest {
                        id: None,
                        name: Some("From".into()),
                        prefix: None,
                        card_prefix: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let beta_sprint_id = beta_sprint["id"].as_str().unwrap().to_string();

        let response = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    board: Some("Beta".into()),
                    sprint: "From".into(),
                }))
                .await
                .unwrap(),
        );
        assert_eq!(response["id"], beta_sprint_id);
    }
}
