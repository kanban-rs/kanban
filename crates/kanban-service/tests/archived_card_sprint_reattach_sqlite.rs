//! F3a (KAN-872): both archived-card sprint mutation sites edit the LIVE card
//! rather than the embedded `Archived::entity` copy or a destructive
//! delete-then-reinsert dance. The re-route is behavior-preserving prep for the
//! F3b `Archived<T>` collapse: it removes a `.entity` consumer and stops abusing
//! the permanent-delete path (`delete_archived_card`) as a transient step.
//!
//! These run through the real SQLite backend on purpose. On SQLite
//! `delete_archived_card` deletes the `cards` row (permanent delete); the old
//! `SetArchivedCardsSprint` dance deleted then reinserted it. Dependency edges
//! are workspace-global and NOT FK-owned by `cards`, so they orphan-survive a
//! card-row delete rather than cascading — these guards pin that the re-routed
//! command leaves the live card's sprint set, the marker intact, and the edges
//! untouched on the backend whose delete semantics differ from in-memory.

use kanban_domain::commands::{CommandContext, SetArchivedCardsSprint};
use kanban_domain::{ArchivedCard, KanbanOperations, KanbanResult};
use kanban_service::{AppConfig, KanbanContext};
use tempfile::TempDir;

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

async fn open(path: &std::path::Path) -> KanbanContext {
    open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap()
}

/// Re-attaching a sprint to an archived card (as `DeleteSprint` undo does) must
/// edit the live card and leave its dependency edges intact.
#[tokio::test(flavor = "multi_thread")]
async fn test_set_archived_cards_sprint_edits_live_card_and_preserves_edges() -> KanbanResult<()> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("reattach.sqlite3");
    let mut ctx = open(&path).await;

    let board = ctx.create_board("Proj".into(), None)?;
    let col = ctx.create_column(board.id, "Todo".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let card_b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;

    let backend = ctx.backend();
    let ds = backend.as_data_store();

    // A blocks B — a workspace-global edge keyed on card id.
    {
        let mut graph = ds.get_graph()?;
        graph.set_block(card_a.id, card_b.id)?;
        ds.set_graph(graph)?;
    }
    assert!(
        ds.get_graph()?.contains(card_a.id, card_b.id),
        "edge A->B seeded"
    );

    // Archive card A (marker over the still-live card). card_a was created via
    // `create_card`, so its live row already exists for the marker's FK.
    ds.insert_archived_card(ArchivedCard::new(card_a.id, board.id))?;

    // Re-attach the sprint (the synthetic inverse command under test).
    let cmd = SetArchivedCardsSprint {
        archived_card_ids: vec![card_a.id],
        sprint_id: sprint.id,
    };
    cmd.execute(&CommandContext { store: ds })?;

    // The LIVE card carries the sprint...
    let live = ds
        .get_card(card_a.id)?
        .expect("archived card A is still live and reachable by id");
    assert_eq!(
        live.sprint_id,
        Some(sprint.id),
        "the live card must carry the re-attached sprint"
    );
    // ...the marker is untouched...
    assert!(
        ds.get_archived_card(card_a.id)?.is_some(),
        "card A must remain archived after re-attach"
    );
    // ...and the dependency edge survived (the regression this card fixes).
    assert!(
        ds.get_graph()?.contains(card_a.id, card_b.id),
        "the archived card's dependency edge must survive sprint re-attach"
    );
    Ok(())
}

/// Clearing a sprint from archived cards edits the live card and touches
/// nothing else. Conformance guard on the divergent backend.
#[tokio::test(flavor = "multi_thread")]
async fn test_clear_sprint_from_archived_cards_edits_live_card() -> KanbanResult<()> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("clear.sqlite3");
    let mut ctx = open(&path).await;

    let board = ctx.create_board("Proj".into(), None)?;
    let col = ctx.create_column(board.id, "Todo".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let card_b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;

    let backend = ctx.backend();
    let ds = backend.as_data_store();

    {
        let mut graph = ds.get_graph()?;
        graph.set_block(card_a.id, card_b.id)?;
        ds.set_graph(graph)?;
    }

    // Bind A to the sprint, then archive it.
    let mut bound = card_a.clone();
    bound.sprint_id = Some(sprint.id);
    ds.upsert_card(bound.clone())?;
    ds.insert_archived_card(ArchivedCard::new(bound.id, board.id))?;

    ds.clear_sprint_from_archived_cards(sprint.id, chrono::Utc::now())?;

    let live = ds
        .get_card(card_a.id)?
        .expect("archived card A is still live");
    assert_eq!(
        live.sprint_id, None,
        "clearing must null the live card's sprint"
    );
    assert!(
        ds.get_archived_card(card_a.id)?.is_some(),
        "card A must remain archived"
    );
    assert!(
        ds.get_graph()?.contains(card_a.id, card_b.id),
        "clearing a sprint must not disturb dependency edges"
    );
    Ok(())
}
