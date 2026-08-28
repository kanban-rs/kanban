use crate::cli::{CardAction, CardCreateArgs, CardListArgs, CardUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{parse_datetime_input, resolve_page_params, PaginatedList};
use kanban_domain::{
    ArchivedFilter, CardListFilter, CardPriority, CardStatus, CardUpdate, CreateCardOptions,
    FieldUpdate, KanbanOperations, SprintStatus,
};
use kanban_service::api::CardResponse;

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
            // I1 (KAN-881): ONE path. The `--archived`/`--include-archived` flags
            // map to the domain archived selector; `list_cards` returns the unified
            // `CardSummary` set (each carrying `archived_at`), live or archived.
            let filter = match build_filter(ctx, &args) {
                Ok(f) => f,
                Err(e) => return output::output_error(&e),
            };
            let summaries = ctx.list_cards(filter)?;
            output::output_success(PaginatedList::paginate(summaries, page, page_size)?);
        }
        CardAction::Get { card } => {
            if let Ok(uuid) = Uuid::parse_str(&card) {
                match ctx.get_card(uuid)? {
                    // Stamp the marker's `archived_at` so an archived card is
                    // not returned looking live (get and list must agree).
                    Some(c) => output::output_success(CardResponse::with_archived_at(
                        &c,
                        ctx.card_archived_at(uuid)?,
                    )),
                    None => return output::output_error(&format!("Card not found: '{}'", card)),
                }
            } else {
                let cards = ctx.find_cards_by_identifier(&card)?;
                match cards.as_slice() {
                    [] => return output::output_error(&format!("Card not found: '{}'", card)),
                    [c] => output::output_success(CardResponse::with_archived_at(
                        c,
                        ctx.card_archived_at(c.id)?,
                    )),
                    _ => {
                        let mut responses: Vec<CardResponse> = Vec::with_capacity(cards.len());
                        for c in cards.iter() {
                            responses.push(CardResponse::with_archived_at(
                                c,
                                ctx.card_archived_at(c.id)?,
                            ));
                        }
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
    // Reference-marker model: an archived card carries its board on the marker
    // (first-class `board_id`), which survives a deleted column. Prefer it; else
    // resolve the LIVE card's column -> board (`get_card` is unfiltered).
    if let Some(marker) = ctx.get_archived_card(card_id).map_err(|e| e.to_string())? {
        return Ok(marker.context.board_id);
    }
    let card = ctx
        .get_card(card_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Card not found: {}", card_id))?;
    let column = ctx
        .get_column(card.column_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Column not found: {}", card.column_id))?;
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
    let archived = if args.archived {
        ArchivedFilter::ArchivedOnly
    } else if args.include_archived {
        ArchivedFilter::Include
    } else {
        ArchivedFilter::LiveOnly
    };
    Ok(CardListFilter {
        board_id,
        column_id,
        sprint_ids: sprint_id.map(|sid| std::iter::once(sid).collect()),
        status,
        sort: args.sort.map(|s| s.to_sort_field()),
        sort_order: args.order.map(|o| o.to_sort_order()),
        archived,
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

pub(crate) fn parse_status(s: &str) -> Result<CardStatus, String> {
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

#[cfg(test)]
mod card_board_id_tests {
    use super::card_board_id;
    use crate::context::CliContext;
    use async_trait::async_trait;
    use kanban_backend::{KanbanBackend, TransactionFn};
    use kanban_backend_memory::InMemoryStore;
    use kanban_domain::command_store::CommandStore;
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{
        ArchivedBoard, ArchivedCard, Board, Card, Column, CommandBatch, DependencyGraph,
        KanbanOperations, KanbanResult, Snapshot, Sprint,
    };
    use kanban_service::{AppConfig, KanbanContext};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use uuid::Uuid;

    #[derive(Default)]
    struct CountingBackend {
        inner: InMemoryStore,
        list_archived_cards_calls: AtomicUsize,
    }

    impl CountingBackend {
        fn list_archived_cards_call_count(&self) -> usize {
            self.list_archived_cards_calls.load(Ordering::SeqCst)
        }
    }

    impl DataStore for CountingBackend {
        fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
            self.inner.get_prefix(name)
        }
        fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
            self.inner.list_prefixes()
        }
        fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
            self.inner.upsert_prefix(prefix)
        }
        fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
            self.inner.get_board(id)
        }
        fn list_boards(&self) -> KanbanResult<Vec<Board>> {
            self.inner.list_boards()
        }
        fn upsert_board(&self, board: Board) -> KanbanResult<()> {
            self.inner.upsert_board(board)
        }
        fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_board(id)
        }
        fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
            self.inner.get_column(id)
        }
        fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
            self.inner.list_columns_by_board(board_id)
        }
        fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
            self.inner.list_all_columns()
        }
        fn upsert_column(&self, column: Column) -> KanbanResult<()> {
            self.inner.upsert_column(column)
        }
        fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_column(id)
        }
        fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_columns_by_board(board_id)
        }
        fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
            self.inner.get_card(id)
        }
        fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
            self.inner.list_all_cards()
        }
        fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_column(column_id)
        }
        fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_sprint(sprint_id)
        }
        fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
            self.inner.count_cards_in_column(column_id)
        }
        fn count_cards_in_column_excluding(
            &self,
            column_id: Uuid,
            exclude: &[Uuid],
        ) -> KanbanResult<usize> {
            self.inner
                .count_cards_in_column_excluding(column_id, exclude)
        }
        fn upsert_card(&self, card: Card) -> KanbanResult<()> {
            self.inner.upsert_card(card)
        }
        fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_card(id)
        }
        fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
            self.inner.delete_cards_by_columns(column_ids)
        }
        fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_columns(column_ids)
        }
        fn list_cards_by_column_filtered(
            &self,
            column_id: Uuid,
            archived: kanban_domain::ArchivedFilter,
        ) -> KanbanResult<Vec<Card>> {
            self.inner
                .list_cards_by_column_filtered(column_id, archived)
        }
        fn count_cards_in_column_filtered(
            &self,
            column_id: Uuid,
            archived: kanban_domain::ArchivedFilter,
        ) -> KanbanResult<usize> {
            self.inner
                .count_cards_in_column_filtered(column_id, archived)
        }
        fn clear_sprint_from_cards(
            &self,
            sprint_id: Uuid,
            timestamp: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            self.inner.clear_sprint_from_cards(sprint_id, timestamp)
        }
        fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
            self.inner.get_archived_card(card_id)
        }
        fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
            self.list_archived_cards_calls
                .fetch_add(1, Ordering::SeqCst);
            self.inner.list_archived_cards()
        }
        fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
            self.inner.insert_archived_card(ac)
        }
        fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_archived_card(card_id)
        }
        fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
            self.inner.list_archived_cards_by_board(board_id)
        }
        fn clear_sprint_from_archived_cards(
            &self,
            sprint_id: Uuid,
            timestamp: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            self.inner
                .clear_sprint_from_archived_cards(sprint_id, timestamp)
        }
        fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
            self.inner.get_archived_board(board_id)
        }
        fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
            self.inner.list_archived_boards()
        }
        fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
            self.inner.insert_archived_board(ab)
        }
        fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_archived_board(board_id)
        }
        fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.unarchive_board(board_id)
        }
        fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
            self.inner.get_sprint(id)
        }
        fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
            self.inner.list_sprints_by_board(board_id)
        }
        fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
            self.inner.list_all_sprints()
        }
        fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
            self.inner.upsert_sprint(sprint)
        }
        fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_sprint(id)
        }
        fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_sprints_by_board(board_id)
        }
        fn get_graph(&self) -> KanbanResult<DependencyGraph> {
            self.inner.get_graph()
        }
        fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
            self.inner.set_graph(graph)
        }
        fn snapshot(&self) -> KanbanResult<Snapshot> {
            self.inner.snapshot()
        }
        fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
            self.inner.apply_snapshot(snapshot)
        }
    }

    impl CommandStore for CountingBackend {
        fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
            self.inner.append_batch(batch)
        }
        fn batch_count(&self) -> KanbanResult<u64> {
            self.inner.batch_count()
        }
        fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>> {
            self.inner.load_batches(from, to)
        }
    }

    #[async_trait]
    impl KanbanBackend for CountingBackend {
        fn as_data_store(&self) -> &dyn DataStore {
            self
        }
        fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
            self.inner.with_transaction(f)
        }
    }

    fn counting_cli_context() -> (Arc<CountingBackend>, CliContext) {
        let backend = Arc::new(CountingBackend::default());
        let ctx = KanbanContext::open_deferred(backend.clone(), AppConfig::default());
        (backend, CliContext::from_context(ctx))
    }

    fn make_card(ctx: &mut CliContext, board_id: Uuid, column_id: Uuid, title: &str) -> Card {
        ctx.create_card(
            board_id,
            column_id,
            title.to_string(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn test_card_board_id_zero_list_archived_cards_calls_for_archived_card() {
        let (backend, mut ctx) = counting_cli_context();
        let board = ctx.create_board("Board".into(), None).unwrap();
        let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
        let card = make_card(&mut ctx, board.id, col.id, "Card");
        ctx.archive_card(card.id).unwrap();

        backend.list_archived_cards_calls.store(0, Ordering::SeqCst);

        let resolved = card_board_id(&ctx, card.id).unwrap();

        assert_eq!(resolved, board.id);
        assert_eq!(
            backend.list_archived_cards_call_count(),
            0,
            "card_board_id should perform a by-id lookup, not a full scan"
        );
    }

    #[test]
    fn test_card_board_id_returns_correct_board_for_live_card() {
        let (_backend, mut ctx) = counting_cli_context();
        let board = ctx.create_board("Board".into(), None).unwrap();
        let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
        let card = make_card(&mut ctx, board.id, col.id, "Card");

        let resolved = card_board_id(&ctx, card.id).unwrap();

        assert_eq!(resolved, board.id);
    }

    #[test]
    fn test_card_board_id_returns_correct_board_for_archived_card() {
        let (_backend, mut ctx) = counting_cli_context();
        let board = ctx.create_board("Board".into(), None).unwrap();
        let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
        let card = make_card(&mut ctx, board.id, col.id, "Card");
        ctx.archive_card(card.id).unwrap();

        let resolved = card_board_id(&ctx, card.id).unwrap();

        assert_eq!(resolved, board.id);
    }

    #[test]
    fn test_card_board_id_returns_error_for_missing_card() {
        let (_backend, ctx) = counting_cli_context();

        let resolved = card_board_id(&ctx, Uuid::new_v4());

        assert!(resolved.is_err());
    }
}
