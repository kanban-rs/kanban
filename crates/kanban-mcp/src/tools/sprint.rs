use crate::helpers::model_read::{resolve_board, resolve_sprint_global, resolve_sprint_in_board};
use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, parse_datetime, project_sprint,
    to_call_tool_result, to_call_tool_result_json,
};
use crate::requests::sprint::{
    ActivateSprintRequest, CancelSprintRequest, CarryOverSprintCardsRequest, CompleteSprintRequest,
    CreateSprintParams, DeleteSprintRequest, GetSprintRequest, ListSprintsRequest,
    UpdateSprintRequest,
};
use crate::scope::{Ref, ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{FieldUpdate, KanbanOperations, SprintUpdate};
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
            sprint: Some(Ref::of(&self.sprint)),
            ..Default::default()
        }
    }
}

impl ToolScoped for UpdateSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.sprint)),
            ..Default::default()
        }
    }
}

impl ToolScoped for ActivateSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.sprint)),
            ..Default::default()
        }
    }
}

impl ToolScoped for CompleteSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.sprint)),
            ..Default::default()
        }
    }
}

impl ToolScoped for CancelSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.sprint)),
            ..Default::default()
        }
    }
}

impl ToolScoped for DeleteSprintRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.sprint)),
            ..Default::default()
        }
    }
}

impl ToolScoped for CarryOverSprintCardsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            sprint: Some(Ref::of(&self.from_sprint)),
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
            let sprint = ctx
                .create_sprint_from_spec(board_id, content.id, content.name, content.prefix)
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
            let model = ctx.model_for(&scope);
            let id = resolve_sprint_global(&model, &req.sprint)?;
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
            name: req.name,
            name_index: FieldUpdate::NoChange,
            prefix: req
                .prefix
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            card_prefix: req
                .card_prefix
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            status: None,
            start_date,
            end_date,
        };
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let id = resolve_sprint_global(&model, &req.sprint)?;
            let sprint = ctx.update_sprint(id, updates).map_err(kanban_err_to_mcp)?;
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
            let model = ctx.model_for(&scope);
            let id = resolve_sprint_global(&model, &req.sprint)?;
            let sprint = ctx
                .activate_sprint(id, req.duration_days)
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
            let model = ctx.model_for(&scope);
            let id = resolve_sprint_global(&model, &req.sprint)?;
            let sprint = ctx.complete_sprint(id).map_err(kanban_err_to_mcp)?;
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
            let model = ctx.model_for(&scope);
            let id = resolve_sprint_global(&model, &req.sprint)?;
            let sprint = ctx.cancel_sprint(id).map_err(kanban_err_to_mcp)?;
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
            let model = ctx.model_for(&scope);
            let id = resolve_sprint_global(&model, &req.sprint)?;
            ctx.delete_sprint(id).map_err(kanban_err_to_mcp)?;
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
            let mut model = ctx.model_for(&scope);
            let from_id = resolve_sprint_global(&model, &req.from_sprint)?;
            let from_sprint = ctx
                .get_sprint(from_id)
                .map_err(kanban_err_to_mcp)?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Sprint not found: {}", from_id), None)
                })?;
            let to_scope = ToolScope {
                sprint: Some(Ref::of(&req.to_sprint)),
                wants_board_sprints: true,
                ..Default::default()
            }
            .for_board(from_sprint.board_id);
            ctx.sync_into(&to_scope, &mut model);
            let to_id = resolve_sprint_in_board(&model, &req.to_sprint, from_sprint.board_id)?;
            ctx.carry_over_sprint_cards(from_id, to_id)
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
    use kanban_persistence_sqlite::{SqliteBackendFactory, SqliteStoreFactory};
    use kanban_service::test_helpers::FaultInjectingBackend;
    use kanban_service::FetchPlan;
    use rmcp::model::ErrorCode;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_sprint_request_scopes_map_names_and_uuids_to_the_right_tiers() {
        let name = GetSprintRequest {
            sprint: "Sprint 1".into(),
        };
        let round = name.scope().next_round(&Model::default());
        assert!(round.board_list);
        assert!(round.sprint_list);

        let id = GetSprintRequest {
            sprint: Uuid::new_v4().to_string(),
        };
        assert!(id.scope().next_round(&Model::default()).is_empty());

        let name = UpdateSprintRequest {
            sprint: "Sprint 1".into(),
            name: None,
            prefix: None,
            card_prefix: None,
            start_date: None,
            end_date: None,
            clear_start_date: None,
            clear_end_date: None,
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let name = ActivateSprintRequest {
            sprint: "Sprint 1".into(),
            duration_days: None,
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let name = CompleteSprintRequest {
            sprint: "Sprint 1".into(),
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let name = CancelSprintRequest {
            sprint: "Sprint 1".into(),
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let name = DeleteSprintRequest {
            sprint: "Sprint 1".into(),
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

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
        from_sprint_id: String,
        to_sprint_id: String,
    }

    async fn seeded_server(file_name: &str) -> Seeded {
        let sqlite_handle = Arc::new(Mutex::new(None));
        let json_handle = Arc::new(Mutex::new(None));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(file_name);

        let server = McpServer::default()
            .register_backend(
                Box::new(SqliteStoreFactory),
                Box::new(RecordingFactory {
                    inner: Box::new(SqliteBackendFactory),
                    handle: Arc::clone(&sqlite_handle),
                }),
            )
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
        let to_sprint = text_payload(
            &server
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
                .unwrap(),
        );

        let from_sprint_id = from_sprint["id"].as_str().unwrap().to_string();
        server
            .tool_activate_sprint(Parameters(ActivateSprintRequest {
                sprint: from_sprint_id.clone(),
                duration_days: None,
            }))
            .await
            .unwrap();
        server
            .tool_complete_sprint(Parameters(CompleteSprintRequest {
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
            from_sprint_id,
            to_sprint_id: to_sprint["id"].as_str().unwrap().to_string(),
        }
    }

    #[tokio::test]
    async fn test_carry_over_sprint_cards_resolves_the_to_sprint_without_a_second_board_read_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
                from_sprint: seeded.from_sprint_id.clone(),
                to_sprint: seeded.to_sprint_id.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("get_board"), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_carry_over_sprint_cards_resolves_the_to_sprint_without_a_second_board_read_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_carry_over_sprint_cards(Parameters(CarryOverSprintCardsRequest {
                from_sprint: seeded.from_sprint_id.clone(),
                to_sprint: seeded.to_sprint_id.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("get_board"), 1);
    }

    #[tokio::test]
    async fn test_sprint_tool_with_an_unloadable_sprint_list_errors_instead_of_reporting_not_found_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_sprints");

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprint list"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sprint_tool_with_an_unloadable_sprint_list_errors_instead_of_reporting_not_found_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_sprints");

        let err = seeded
            .server
            .tool_get_sprint(Parameters(GetSprintRequest {
                sprint: "From".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprint list"));
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

        let updated = text_payload(
            &seeded
                .server
                .tool_update_sprint(Parameters(UpdateSprintRequest {
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
                sprint: "Renamed".into(),
            }))
            .await
            .unwrap();

        let deleted = text_payload(
            &seeded
                .server
                .tool_delete_sprint(Parameters(DeleteSprintRequest {
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
    async fn test_get_sprint_result_json_is_unchanged() {
        let seeded = seeded_server("test.json").await;

        let response = text_payload(
            &seeded
                .server
                .tool_get_sprint(Parameters(GetSprintRequest {
                    sprint: "From".into(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["name"], "From");
        assert!(response["id"].is_string());
        assert!(response["sprint_number"].is_number());
    }
}
