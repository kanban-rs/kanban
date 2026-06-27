use crate::cli::{ExportArgs, ImportArgs};
use crate::context::CliContext;
use crate::output;
use kanban_domain::KanbanOperations;
use kanban_service::api::BoardResponse;

pub async fn handle_export(ctx: &CliContext, args: ExportArgs) -> anyhow::Result<()> {
    let board_uuid = match args.board {
        Some(raw) => match ctx.resolve_board_id(&raw) {
            Ok(u) => Some(u),
            Err(e) => return output::output_error(&e.to_string()),
        },
        None => None,
    };
    let json = ctx.export_board(board_uuid)?;
    println!("{}", json);
    Ok(())
}

pub async fn handle_import(ctx: &mut CliContext, args: ImportArgs) -> anyhow::Result<()> {
    let data = std::fs::read_to_string(&args.file)
        .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", args.file, e))?;
    let board = ctx.import_board(&data)?;
    ctx.save().await?;
    output::output_success(BoardResponse::from(&board));
    Ok(())
}
