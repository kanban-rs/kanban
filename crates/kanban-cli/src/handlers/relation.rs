use crate::cli::{RelationAction, SortDir, SortKey};
use crate::context::CliContext;
use crate::error::{KanbanCliError, KanbanCliResult};
use crate::output;
use kanban_domain::dependencies::messages;
use kanban_domain::error::{DependencyError, DomainError};
use kanban_domain::sort::OrderedSorter;
use kanban_domain::{Card, CardSummary, GraphOperations, KanbanError, KanbanOperations};
use uuid::Uuid;

fn resolve_cards(ctx: &CliContext, ids: Vec<Uuid>) -> Vec<Card> {
    ids.into_iter()
        .filter_map(|id| match ctx.get_card(id) {
            Ok(Some(card)) => Some(card),
            Ok(None) => {
                tracing::warn!(
                    "graph references unknown card id {id}; dropping from list (possible corruption)"
                );
                None
            }
            Err(e) => {
                tracing::warn!("failed to resolve card id {id}: {e}; dropping from list");
                None
            }
        })
        .collect()
}

fn sort_and_summarize(mut cards: Vec<Card>, sort: SortKey, order: SortDir) -> Vec<CardSummary> {
    let sorter = OrderedSorter::new(sort.to_sort_by(), order.to_sort_order());
    sorter.sort_by(&mut cards);
    cards.iter().map(CardSummary::from).collect()
}

/// Resolve every raw child identifier into a UUID, short-circuiting on
/// the first resolution failure. The atomic batch then sees a list of
/// already-validated UUIDs — failures here use the raw identifier the
/// user supplied so the error breadcrumb stays meaningful.
fn resolve_children(ctx: &CliContext, raw: &[String]) -> KanbanCliResult<Vec<Uuid>> {
    raw.iter()
        .map(|r| ctx.resolve_card_id(r).map_err(Into::into))
        .collect()
}

/// Batch-mode enrichment for `attach_children`. The atomic batch returns
/// a single error rather than the per-child error the loop-of-singles
/// produced, so the hint identifies the parent and the offending child
/// where the variant lets us narrow it down:
/// - `SelfReference`: the offending child is exactly the one whose
///   resolved UUID matches the parent — we name that raw identifier.
/// - `CycleDetected`: any child in the list could be the culprit; we
///   list them all so the user can see the entries involved without
///   needing to bisect.
fn enrich_add_error_for_batch(
    e: KanbanError,
    parent: &str,
    children_raw: &[String],
) -> KanbanCliError {
    match e {
        KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected)) => {
            let hint = if let [only] = children_raw {
                messages::parent_cycle(parent, only)
            } else {
                format!(
                    "cycle detected: making {parent} a parent of one of [{}] would create a cycle",
                    children_raw.join(", ")
                )
            };
            KanbanCliError::Resolution { hint }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::SelfReference)) => {
            KanbanCliError::Resolution {
                hint: messages::parent_self_reference(parent),
            }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::DuplicateEdge)) => {
            let hint = if let [only] = children_raw {
                messages::parent_duplicate(parent, only)
            } else {
                format!(
                    "edge already exists: one of [{}] is already a child of {parent}",
                    children_raw.join(", ")
                )
            };
            KanbanCliError::Resolution { hint }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::EdgeNotFound)) => e.into(),
        other => other.into(),
    }
}

/// Batch-mode enrichment for `detach_children`. `EdgeNotFound` is the
/// only reachable dependency-variant failure on a remove: the hint
/// names the parent and the children list so the user can see which
/// invocation failed without re-reading scrollback.
fn enrich_remove_error_for_batch(
    e: KanbanError,
    parent: &str,
    children_raw: &[String],
) -> KanbanCliError {
    match e {
        KanbanError::Domain(DomainError::Dependency(DependencyError::EdgeNotFound)) => {
            let hint = if let [only] = children_raw {
                messages::parent_edge_not_found(parent, only)
            } else {
                format!(
                    "edge not found: no parent->child edge from {parent} to any of [{}] to remove",
                    children_raw.join(", ")
                )
            };
            KanbanCliError::Resolution { hint }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected)) => e.into(),
        KanbanError::Domain(DomainError::Dependency(DependencyError::SelfReference)) => e.into(),
        KanbanError::Domain(DomainError::Dependency(DependencyError::DuplicateEdge)) => e.into(),
        other => other.into(),
    }
}

pub async fn handle(ctx: &mut CliContext, action: RelationAction) -> anyhow::Result<()> {
    let result: KanbanCliResult<serde_json::Value> = run(ctx, action).await;
    match result {
        Ok(value) => {
            output::output_success(value);
            Ok(())
        }
        Err(e) => output::output_error(&e.to_string()),
    }
}

async fn run(ctx: &mut CliContext, action: RelationAction) -> KanbanCliResult<serde_json::Value> {
    match action {
        RelationAction::Add { parent, children } => {
            // Resolve raw identifiers up front, then commit all
            // edges in a single atomic batch via `attach_children`.
            // The service rolls the entire batch back on any failure
            // (cycle / self-ref / unknown card), so a mid-list error
            // never leaves a partial state in memory or on disk.
            let parent_uuid = ctx.resolve_card_id(&parent)?;
            let child_uuids = resolve_children(ctx, &children)?;
            let response = serde_json::json!({
                "parent":   parent_uuid.to_string(),
                "children": serde_json::to_value(&child_uuids)?,
            });
            ctx.mutate_unit(|c| c.attach_children_impl(parent_uuid, child_uuids))
                .map_err(|e| enrich_add_error_for_batch(e, &parent, &children))?;
            ctx.save().await?;
            Ok(response)
        }
        RelationAction::Remove { parent, children } => {
            let parent_uuid = ctx.resolve_card_id(&parent)?;
            let child_uuids = resolve_children(ctx, &children)?;
            let response = serde_json::json!({
                "parent":   parent_uuid.to_string(),
                "children": serde_json::to_value(&child_uuids)?,
            });
            ctx.mutate_unit(|c| c.detach_children_impl(parent_uuid, child_uuids))
                .map_err(|e| enrich_remove_error_for_batch(e, &parent, &children))?;
            ctx.save().await?;
            Ok(response)
        }
        RelationAction::Parents { card, sort, order } => {
            let uuid = ctx.resolve_card_id(&card)?;
            let ids = ctx.list_parents_of(uuid)?;
            let cards = resolve_cards(ctx, ids);
            Ok(serde_json::to_value(sort_and_summarize(
                cards, sort, order,
            ))?)
        }
        RelationAction::Children { card, sort, order } => {
            let uuid = ctx.resolve_card_id(&card)?;
            let ids = ctx.list_children_of(uuid)?;
            let cards = resolve_cards(ctx, ids);
            Ok(serde_json::to_value(sort_and_summarize(
                cards, sort, order,
            ))?)
        }
    }
}
