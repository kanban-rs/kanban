use crate::cli::{CardAction, CardCreateArgs, CardListArgs, CardUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{parse_datetime_input, resolve_page_params, PaginatedList};
use kanban_domain::{
    CardListFilter, CardPriority, CardStatus, CardUpdate, CreateCardOptions, FieldUpdate,
    KanbanOperations, SprintStatus,
};
use kanban_service::api::{ArchivedCardResponse, CardResponse};

use uuid::Uuid;

pub async fn handle(ctx: &mut CliContext, action: CardAction) -> anyhow::Result<()> {
    match action {
        CardAction::Create(args) => {
            let board_uuid = match ctx.resolve_board_id(&args.board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let column_uuid = match ctx.resolve_column_id(&args.column, board_uuid) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let sprint_uuid = match resolve_assign_sprint(ctx, board_uuid, &args.assign_sprint) {
                Ok(s) => s,
                Err(e) => return output::output_error(&e),
            };
            let mut options = match build_create_options(&args) {
                Ok(o) => o,
                Err(e) => return output::output_error(&e),
            };
            options.sprint_id = sprint_uuid;
            // Funnels through the Card factory via the create command (KAN-796);
            // the JSON edge projects the domain Card via CardResponse.
            let card = ctx.create_card(board_uuid, column_uuid, args.title, options)?;
            ctx.save().await?;
            output::output_success(CardResponse::from(&card));
        }
        CardAction::List(args) => {
            let (page, page_size) = resolve_page_params(args.page, args.page_size)?;
            if args.archived {
                let board_id = match &args.board {
                    Some(raw) => match ctx.resolve_board_id(raw) {
                        Ok(u) => Some(u),
                        Err(e) => return output::output_error(&e.to_string()),
                    },
                    None => None,
                };
                let archived =
                    ctx.list_archived_cards_sorted(kanban_domain::ArchivedCardListFilter {
                        board_id,
                        sort: args.sort.map(|s| s.to_sort_field()),
                        sort_order: args.order.map(|o| o.to_sort_order()),
                    })?;
                let responses: Vec<ArchivedCardResponse> =
                    archived.iter().map(ArchivedCardResponse::from).collect();
                output::output_success(PaginatedList::paginate(responses, page, page_size)?);
            } else {
                let filter = match build_filter(ctx, &args) {
                    Ok(f) => f,
                    Err(e) => return output::output_error(&e),
                };
                let summaries = ctx.list_cards(filter)?;
                output::output_success(PaginatedList::paginate(summaries, page, page_size)?);
            }
        }
        CardAction::Get { card } => {
            if let Ok(uuid) = Uuid::parse_str(&card) {
                match ctx.get_card(uuid)? {
                    Some(c) => output::output_success(CardResponse::from(&c)),
                    None => return output::output_error(&format!("Card not found: '{}'", card)),
                }
            } else {
                let cards = ctx.find_cards_by_identifier(&card)?;
                match cards.as_slice() {
                    [] => return output::output_error(&format!("Card not found: '{}'", card)),
                    [c] => output::output_success(CardResponse::from(c)),
                    _ => {
                        let responses: Vec<CardResponse> =
                            cards.iter().map(CardResponse::from).collect();
                        output::output_success(&responses)
                    }
                }
            }
        }
        CardAction::Update(args) => {
            let uuid = match ctx.resolve_card_id(&args.card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let updates = match build_card_update(&args) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            let card = ctx.update_card(uuid, updates)?;
            ctx.save().await?;
            output::output_success(CardResponse::from(&card));
        }
        CardAction::Move {
            card,
            column,
            position,
        } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let column_uuid = match resolve_column_for_card(ctx, &column, uuid) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            let moved = ctx.move_card(uuid, column_uuid, position)?;
            ctx.save().await?;
            output::output_success(CardResponse::from(&moved));
        }
        CardAction::Archive { card } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.archive_card(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"archived": uuid.to_string()}));
        }
        CardAction::Restore { card, column } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let column_uuid = match column {
                Some(raw) => match resolve_column_for_card(ctx, &raw, uuid) {
                    Ok(u) => Some(u),
                    Err(e) => return output::output_error(&e),
                },
                None => None,
            };
            let restored = ctx.restore_card(uuid, column_uuid)?;
            ctx.save().await?;
            output::output_success(CardResponse::from(&restored));
        }
        CardAction::Delete { card } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.delete_card(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
        CardAction::AssignSprint { card, sprint } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let sprint_uuid = match resolve_sprint_for_card(ctx, &sprint, uuid) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            let assigned = ctx.assign_card_to_sprint(uuid, sprint_uuid)?;
            ctx.save().await?;
            output::output_success(CardResponse::from(&assigned));
        }
        CardAction::UnassignSprint { card } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let unassigned = ctx.unassign_card_from_sprint(uuid)?;
            ctx.save().await?;
            output::output_success(CardResponse::from(&unassigned));
        }
        CardAction::BranchName { card } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let branch = ctx.get_card_branch_name(uuid)?;
            output::output_success(serde_json::json!({"branch_name": branch}));
        }
        CardAction::GitCheckout { card } => {
            let uuid = match ctx.resolve_card_id(&card) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let cmd = ctx.get_card_git_checkout(uuid)?;
            output::output_success(serde_json::json!({"command": cmd}));
        }
        CardAction::ArchiveCards { cards } => {
            let uuids = match ctx.resolve_card_ids(&cards) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let result = ctx.archive_cards_detailed(uuids);
            ctx.save().await?;
            output::output_success(serde_json::json!({
                "succeeded_count": result.succeeded.len(),
                "failed_count": result.failed.len(),
                "succeeded": result.succeeded,
                "failed": result.failed
            }));
        }
        CardAction::MoveCards { cards, column } => {
            let uuids = match ctx.resolve_card_ids(&cards) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let shared_board = match ctx.require_same_board(&uuids) {
                Ok(b) => b,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let column_uuid = match ctx.resolve_column_id(&column, shared_board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let result = ctx.move_cards_detailed(uuids, column_uuid);
            ctx.save().await?;
            output::output_success(serde_json::json!({
                "succeeded_count": result.succeeded.len(),
                "failed_count": result.failed.len(),
                "succeeded": result.succeeded,
                "failed": result.failed
            }));
        }
        CardAction::AssignCardsToSprint { cards, sprint } => {
            let uuids = match ctx.resolve_card_ids(&cards) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let shared_board = match ctx.require_same_board(&uuids) {
                Ok(b) => b,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let sprint_uuid = match ctx.resolve_sprint_id(&sprint, shared_board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            let result = ctx.assign_cards_to_sprint_detailed(uuids, sprint_uuid);
            ctx.save().await?;
            output::output_success(serde_json::json!({
                "succeeded_count": result.succeeded.len(),
                "failed_count": result.failed.len(),
                "succeeded": result.succeeded,
                "failed": result.failed
            }));
        }
    }
    Ok(())
}

fn resolve_column_for_card(ctx: &CliContext, raw: &str, card_id: Uuid) -> Result<Uuid, String> {
    let board_id = card_board_id(ctx, card_id)?;
    ctx.resolve_column_id(raw, board_id)
        .map_err(|e| e.to_string())
}

fn resolve_sprint_for_card(ctx: &CliContext, raw: &str, card_id: Uuid) -> Result<Uuid, String> {
    let board_id = card_board_id(ctx, card_id)?;
    ctx.resolve_sprint_id(raw, board_id)
        .map_err(|e| e.to_string())
}

fn card_board_id(ctx: &CliContext, card_id: Uuid) -> Result<Uuid, String> {
    // Try active cards first; if the card is archived, fall back via its
    // original_column_id. Either path resolves to the card's board.
    let column_id = match ctx.get_card(card_id).map_err(|e| e.to_string())? {
        Some(card) => card.column_id,
        None => {
            let archived = ctx
                .list_archived_cards()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|a| a.card.id == card_id)
                .ok_or_else(|| format!("Card not found: {}", card_id))?;
            archived.original_column_id
        }
    };
    let column = ctx
        .get_column(column_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Column not found: {}", column_id))?;
    Ok(column.board_id)
}

fn build_filter(ctx: &CliContext, args: &CardListArgs) -> Result<CardListFilter, String> {
    let status = match &args.status {
        Some(s) => Some(parse_status(s)?),
        None => None,
    };
    let board_id = match &args.board {
        Some(raw) => Some(ctx.resolve_board_id(raw).map_err(|e| e.to_string())?),
        None => None,
    };
    let column_id = match &args.column {
        Some(raw) => Some(match board_id {
            Some(bid) => ctx.resolve_column_id(raw, bid).map_err(|e| e.to_string())?,
            None => ctx
                .resolve_column_id_global(raw)
                .map_err(|e| e.to_string())?,
        }),
        None => None,
    };
    let sprint_id = match &args.sprint {
        Some(raw) => Some(match board_id {
            Some(bid) => ctx.resolve_sprint_id(raw, bid).map_err(|e| e.to_string())?,
            None => ctx
                .resolve_sprint_id_global(raw)
                .map_err(|e| e.to_string())?,
        }),
        None => None,
    };
    Ok(CardListFilter {
        board_id,
        column_id,
        sprint_ids: sprint_id.map(|sid| std::iter::once(sid).collect()),
        status,
        sort: args.sort.map(|s| s.to_sort_field()),
        sort_order: args.order.map(|o| o.to_sort_order()),
        ..Default::default()
    })
}

fn resolve_assign_sprint(
    ctx: &CliContext,
    board_id: Uuid,
    flag: &Option<String>,
) -> Result<Option<Uuid>, String> {
    let raw = match flag {
        None => return Ok(None),
        Some(s) => s.as_str(),
    };
    if raw.is_empty() {
        let now = chrono::Utc::now();
        let sprints = ctx.list_sprints(board_id).map_err(|e| e.to_string())?;
        let active: Vec<_> = sprints
            .iter()
            .filter(|s| s.status == SprintStatus::Active && !s.is_ended(now))
            .collect();
        match active.as_slice() {
            [] => Err("--assign with no value requires exactly one active sprint on the board; found none".to_string()),
            [s] => Ok(Some(s.id)),
            many => Err(format!(
                "--assign with no value requires exactly one active sprint; found {}",
                many.len()
            )),
        }
    } else {
        ctx.resolve_sprint_id(raw, board_id)
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

fn build_create_options(args: &CardCreateArgs) -> Result<CreateCardOptions, String> {
    let priority = match &args.priority {
        Some(p) => Some(parse_priority(p)?),
        None => None,
    };
    let due_date = match &args.due_date {
        Some(d) => Some(parse_datetime_input(d)?),
        None => None,
    };
    Ok(CreateCardOptions {
        description: args.description.clone(),
        priority,
        points: args.points,
        due_date,
        ..Default::default()
    })
}

fn build_card_update(args: &CardUpdateArgs) -> Result<CardUpdate, String> {
    let priority = match &args.priority {
        Some(p) => Some(parse_priority(p)?),
        None => None,
    };
    let status = match &args.status {
        Some(s) => Some(parse_status(s)?),
        None => None,
    };
    Ok(CardUpdate {
        title: args.title.clone(),
        description: args
            .description
            .clone()
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        priority,
        status,
        position: None,
        column_id: None,
        points: args
            .points
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        due_date: if args.clear_due_date {
            FieldUpdate::Clear
        } else {
            match &args.due_date {
                Some(d) => FieldUpdate::Set(parse_datetime_input(d)?),
                None => FieldUpdate::NoChange,
            }
        },
        sprint_id: FieldUpdate::NoChange,
    })
}

fn parse_priority(s: &str) -> Result<CardPriority, String> {
    match s.to_lowercase().as_str() {
        "low" => Ok(CardPriority::Low),
        "medium" => Ok(CardPriority::Medium),
        "high" => Ok(CardPriority::High),
        "critical" => Ok(CardPriority::Critical),
        _ => Err(format!(
            "Invalid priority '{}'. Valid values: low, medium, high, critical",
            s
        )),
    }
}

fn parse_status(s: &str) -> Result<CardStatus, String> {
    match s.to_lowercase().replace(['-', '_'], "").as_str() {
        "todo" => Ok(CardStatus::Todo),
        "inprogress" => Ok(CardStatus::InProgress),
        "blocked" => Ok(CardStatus::Blocked),
        "done" => Ok(CardStatus::Done),
        _ => Err(format!(
            "Invalid status '{}'. Valid values: todo, in-progress, blocked, done",
            s
        )),
    }
}
