use crate::cli::{SprintAction, SprintUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{parse_datetime_input, resolve_page_params, PaginatedList};
use kanban_domain::{FieldUpdate, KanbanOperations, Sprint, SprintUpdate};
use kanban_service::api::SprintResponse;
use kanban_service::resolve_sprint_name;

/// Project a domain `Sprint` into its wire `SprintResponse`, resolving the
/// `name` against the owning board via the shared service helper so the JSON
/// edge never leaks the internal `name_index` or non-snake-case enum reprs.
fn sprint_response(ctx: &CliContext, sprint: &Sprint) -> anyhow::Result<SprintResponse> {
    let name = resolve_sprint_name(ctx, sprint)?;
    Ok(SprintResponse::new(sprint, name))
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
            let sprint = ctx.mutate(|c| c.create_sprint_impl(board_uuid, prefix, name))?;
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
            let names = kanban_service::resolve_sprint_names(ctx, board_uuid, &sprints)?;
            let responses: Vec<SprintResponse> = sprints
                .iter()
                .zip(names)
                .map(|(s, name)| SprintResponse::new(s, name))
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
            let activated = ctx.mutate(|c| c.activate_sprint_impl(uuid, duration_days))?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &activated)?);
        }
        SprintAction::Complete { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let completed = ctx.mutate(|c| c.complete_sprint_impl(uuid))?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &completed)?);
        }
        SprintAction::Cancel { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let cancelled = ctx.mutate(|c| c.cancel_sprint_impl(uuid))?;
            ctx.save().await?;
            output::output_success(sprint_response(ctx, &cancelled)?);
        }
        SprintAction::Delete { sprint } => {
            let uuid = match ctx.resolve_sprint_id_global(&sprint) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.mutate_unit(|c| c.delete_sprint_impl(uuid))?;
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
            let count = ctx.mutate(|c| c.carry_over_sprint_cards_impl(from_uuid, to_uuid))?;
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
    let sprint = ctx.mutate(|c| c.update_sprint_impl(uuid, updates))?;
    ctx.save().await?;
    Ok(sprint)
}
