use crate::cli::{SprintAction, SprintUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{parse_datetime_input, resolve_page_params, PaginatedList};
use kanban_domain::{FieldUpdate, KanbanOperations, Sprint, SprintUpdate};
use kanban_service::api::SprintResponse;

/// Project a domain `Sprint` into its wire `SprintResponse`, resolving the
/// `name` against the owning board (fetched here) so the JSON edge never leaks
/// the internal `name_index` or non-snake-case enum reprs.
fn sprint_response(ctx: &CliContext, sprint: &Sprint) -> anyhow::Result<SprintResponse> {
    let board = ctx
        .get_board(sprint.board_id)?
        .ok_or_else(|| anyhow::anyhow!("Board not found: {}", sprint.board_id))?;
    Ok(SprintResponse::from_sprint(sprint, &board))
}

pub async fn handle(ctx: &mut CliContext, action: SprintAction) -> anyhow::Result<()> {
    match action {
        SprintAction::Create {
            board,
            prefix,
            name,
        } => {
            let board_uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            // Funnels through the Sprint factory via the create command
            // (KAN-798); the JSON edge projects the domain Sprint via
            // SprintResponse, resolving the sprint name against its owning board.
            let sprint = ctx.create_sprint(board_uuid, prefix, name)?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &sprint)?);
        }
        SprintAction::List {
            board,
            page,
            page_size,
        } => {
            let board_uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let sprints = ctx.list_sprints(board_uuid)?;
            let board = ctx
                .get_board(board_uuid)?
                .ok_or_else(|| anyhow::anyhow!("Board not found: {}", board_uuid))?;
            let responses: Vec<SprintResponse> = sprints
                .iter()
                .map(|s| SprintResponse::from_sprint(s, &board))
                .collect();
            let (page, page_size) = resolve_page_params(page, page_size)?;
            output::output_success(PaginatedList::paginate(responses, page, page_size)?);
        }
        SprintAction::Get { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            match ctx.get_sprint(uuid)? {
                Some(s) => output::output_success(sprint_response(ctx, &s)?),
                None => return output::output_error(&format!("Sprint not found: {}", sprint)),
            }
        }
        SprintAction::Update(args) => {
            let sprint = match handle_update(ctx, args).await {
                Ok(s) => s,
                Err(e) => return output::output_error(&e.to_string()),
            };
            output::output_success(sprint_response(ctx, &sprint)?);
        }
        SprintAction::Activate {
            sprint,
            duration_days,
        } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let activated = ctx.activate_sprint(uuid, duration_days)?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &activated)?);
        }
        SprintAction::Complete { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let completed = ctx.complete_sprint(uuid)?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &completed)?);
        }
        SprintAction::Cancel { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let cancelled = ctx.cancel_sprint(uuid)?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &cancelled)?);
        }
        SprintAction::Delete { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.delete_sprint(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
        SprintAction::CarryOver { from, to } => {
            let from_uuid = match ctx.resolve_sprint_id_global(&from) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            // `--to` is scoped to the same board as `--from`.
            let from_sprint = ctx
                .get_sprint(from_uuid)?
                .ok_or_else(|| anyhow::anyhow!("Source sprint not found: {}", from_uuid))?;
            let to_uuid = match ctx.resolve_sprint_id(&to, from_sprint.board_id) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let count = ctx.carry_over_sprint_cards(from_uuid, to_uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({ "carried_over": count }));
        }
    }
    Ok(())
}

async fn handle_update(
    ctx: &mut CliContext,
    args: SprintUpdateArgs,
) -> anyhow::Result<kanban_domain::Sprint> {
    let uuid = ctx
        .resolve_sprint_id_global(&args.sprint)
        .map_err(anyhow::Error::from)?;
    let start_date = if args.clear_start_date {
        FieldUpdate::Clear
    } else {
        match args.start_date {
            Some(d) => FieldUpdate::Set(parse_datetime_input(&d).map_err(anyhow::Error::msg)?),
            None => FieldUpdate::NoChange,
        }
    };

    let end_date = if args.clear_end_date {
        FieldUpdate::Clear
    } else {
        match args.end_date {
            Some(d) => FieldUpdate::Set(parse_datetime_input(&d).map_err(anyhow::Error::msg)?),
            None => FieldUpdate::NoChange,
        }
    };

    let updates = SprintUpdate {
        name: args.name,
        name_index: FieldUpdate::NoChange,
        prefix: args
            .prefix
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        card_prefix: args
            .card_prefix
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        status: None,
        start_date,
        end_date,
    };
    let sprint = ctx.update_sprint(uuid, updates)?;
    ctx.save().await?;
    Ok(sprint)
}
