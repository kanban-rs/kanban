use crate::cli::{ColumnAction, ColumnUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{ColumnUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::ColumnResponse;

pub async fn handle(ctx: &mut CliContext, action: ColumnAction) -> anyhow::Result<()> {
    match action {
        ColumnAction::Create {
            board,
            name,
            position,
        } => {
            let board_uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            // Funnels through the Column factory via the board_id/name/position
            // shim (KAN-794); the JSON edge projects the domain Column via
            // ColumnResponse.
            let column = ctx.create_column(board_uuid, name, position)?;
            ctx.save().await?;
            output::output_success(ColumnResponse::from(&column));
        }
        ColumnAction::List {
            board,
            page,
            page_size,
        } => {
            let board_uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let columns = ctx.list_columns(board_uuid)?;
            let (page, page_size) = resolve_page_params(page, page_size)?;
            output::output_success(PaginatedList::paginate(columns, page, page_size)?);
        }
        ColumnAction::Get { column } => {
            let uuid = match ctx.resolve_column_id_global(&column) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            match ctx.get_column(uuid)? {
                Some(c) => output::output_success(ColumnResponse::from(&c)),
                None => return output::output_error(&format!("Column not found: {}", column)),
            }
        }
        ColumnAction::Update(args) => {
            let column = handle_update(ctx, args).await?;
            output::output_success(ColumnResponse::from(&column));
        }
        ColumnAction::Delete { column } => {
            let uuid = match ctx.resolve_column_id_global(&column) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.delete_column(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
        ColumnAction::Reorder { column, position } => {
            let uuid = match ctx.resolve_column_id_global(&column) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let c = ctx.reorder_column(uuid, position)?;
            ctx.save().await?;
            output::output_success(ColumnResponse::from(&c));
        }
    }
    Ok(())
}

async fn handle_update(
    ctx: &mut CliContext,
    args: ColumnUpdateArgs,
) -> anyhow::Result<kanban_domain::Column> {
    let uuid = ctx
        .resolve_column_id_global(&args.column)
        .map_err(anyhow::Error::from)?;
    let updates = ColumnUpdate {
        name: args.name,
        position: args.position,
        wip_limit: if args.clear_wip_limit {
            FieldUpdate::Clear
        } else {
            args.wip_limit
                .map(|w| w as i32)
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange)
        },
    };
    let column = ctx.update_column(uuid, updates)?;
    ctx.save().await?;
    Ok(column)
}
