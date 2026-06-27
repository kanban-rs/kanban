use crate::cli::{BoardAction, BoardUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{BoardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::BoardResponse;

pub async fn handle(ctx: &mut CliContext, action: BoardAction) -> anyhow::Result<()> {
    match action {
        BoardAction::Create { name, card_prefix } => {
            // Funnels through the Board factory via the name/card_prefix shim
            // (KAN-792); the JSON edge projects the domain Board via BoardResponse.
            let board = ctx.create_board(name, card_prefix)?;
            ctx.save().await?;
            output::output_success(BoardResponse::from(&board));
        }
        BoardAction::List { page, page_size } => {
            let boards = ctx.list_boards()?;
            let responses: Vec<BoardResponse> = boards.iter().map(BoardResponse::from).collect();
            let (page, page_size) = resolve_page_params(page, page_size)?;
            output::output_success(PaginatedList::paginate(responses, page, page_size)?);
        }
        BoardAction::Get { board } => {
            let uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            match ctx.get_board(uuid)? {
                Some(b) => output::output_success(BoardResponse::from(&b)),
                None => return output::output_error(&format!("Board not found: {}", board)),
            }
        }
        BoardAction::Update(args) => {
            let board = handle_update(ctx, args).await?;
            output::output_success(BoardResponse::from(&board));
        }
        BoardAction::Delete { board } => {
            let uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.delete_board(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
    }
    Ok(())
}

async fn handle_update(
    ctx: &mut CliContext,
    args: BoardUpdateArgs,
) -> anyhow::Result<kanban_domain::Board> {
    let uuid = ctx
        .resolve_board_id(&args.board)
        .map_err(anyhow::Error::from)?;
    let updates = BoardUpdate {
        name: args.name,
        description: args
            .description
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        sprint_prefix: args
            .sprint_prefix
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        card_prefix: args
            .card_prefix
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        task_sort_field: args.sort_field.map(|s| s.to_sort_field()),
        task_sort_order: args.sort_order.map(|o| o.to_sort_order()),
        ..Default::default()
    };
    let board = ctx.update_board(uuid, updates)?;
    ctx.save().await?;
    Ok(board)
}
