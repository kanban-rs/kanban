use crate::cli::LabelAction;
use crate::context::CliContext;
use crate::output;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{KanbanOperations, Label};

pub async fn handle(ctx: &mut CliContext, action: LabelAction) -> anyhow::Result<()> {
    match action {
        LabelAction::List { page, page_size } => {
            let labels = ctx.list_labels()?;
            let (page, page_size) = resolve_page_params(page, page_size)?;
            let paged = PaginatedList::paginate(labels, page, page_size)?;
            output::output_success(paged);
        }
        LabelAction::Get { name } => match ctx.get_label(&name)? {
            Some(label) => output::output_success(label),
            None => return output::output_error(&format!("Label not found: {name}")),
        },
        LabelAction::Upsert {
            name,
            color,
            description,
        } => {
            let label = Label {
                name,
                color,
                description,
            };
            let saved = ctx.upsert_label(label)?;
            ctx.save().await?;
            output::output_success(saved);
        }
        LabelAction::Delete { name } => {
            ctx.delete_label(name)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({ "deleted": true }));
        }
    }
    Ok(())
}
