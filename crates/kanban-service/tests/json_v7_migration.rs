//! End-to-end JSON legacy-file migration (V7 -> V8) coexisting with the
//! archived-board feature (C1-C4). A V7 envelope predates both
//! `archived_cards.board_id` (V8 backfill) and the `archived_boards` collection.
//! Loading it must: run the V7->V8 chain, backfill `board_id` on archived
//! cards, default `archived_boards` to empty, preserve live/board data, and
//! leave the store fully able to archive a board afterward.
//!
//! Fixtures are built by saving a real (current) file and downgrading it, so
//! the envelope is guaranteed structurally valid.

use kanban_domain::{DataStore, KanbanOperations, KanbanResult};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

/// Rewrite a current (marker-shape) file to a GENUINE legacy V7 file so the full
/// V7 -> V8 -> V9 chain plus the F3b read-shim are exercised end-to-end. A V7
/// archived card EMBEDS its entity (under the legacy `card` key — what V7 -> V8
/// reads), is NOT present in the live `cards` array, and carries
/// `original_column_id` / `original_position` (from which V7 -> V8 backfills
/// `board_id`) but no `board_id` and no `entity_id`. Also bumps the version down
/// and removes the `archived_boards` key (V7 predates that collection).
fn downgrade_to_v7(path: &std::path::Path) {
    let raw = std::fs::read_to_string(path).unwrap();
    let mut env: serde_json::Value = serde_json::from_str(&raw).unwrap();
    env["version"] = serde_json::json!(7);
    let data = env["data"].as_object_mut().unwrap();
    data.remove("archived_boards");

    // The marker-shape file keeps every card (live + archived) in `cards` and a
    // pure marker under `archived_cards`. Reconstruct the V7 embed shape: MOVE
    // each archived marker's live card row out of `cards` and embed it.
    let archived_entity_ids: Vec<String> = data
        .get("archived_cards")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|ac| {
            ac.get("entity_id")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();

    let mut embedded: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    if let Some(cards) = data.get_mut("cards").and_then(|v| v.as_array_mut()) {
        cards.retain(|c| match c.get("id").and_then(|v| v.as_str()) {
            Some(id) if archived_entity_ids.iter().any(|a| a == id) => {
                embedded.insert(id.to_string(), c.clone());
                false // V7 did NOT carry the archived card in the live array
            }
            _ => true,
        });
    }

    if let Some(acs) = data
        .get_mut("archived_cards")
        .and_then(|v| v.as_array_mut())
    {
        for ac in acs {
            let obj = ac.as_object_mut().unwrap();
            let entity_id = obj
                .get("entity_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            if let Some(card) = entity_id.as_deref().and_then(|id| embedded.get(id)) {
                let column_id = card
                    .get("column_id")
                    .cloned()
                    .unwrap_or(serde_json::json!(null));
                let position = card
                    .get("position")
                    .cloned()
                    .unwrap_or(serde_json::json!(0));
                obj.insert("card".to_string(), card.clone());
                obj.insert("original_column_id".to_string(), column_id);
                obj.insert("original_position".to_string(), position);
            }
            // V7 carried neither of these on an archived card.
            obj.remove("board_id");
            obj.remove("entity_id");
        }
    }
    std::fs::write(path, serde_json::to_string(&env).unwrap()).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_v7_json_migrates_and_preserves_cards_and_boards() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("legacy.json");

    // --- Build a realistic current file: 2 boards, columns, live card + an
    //     archived card, then save. ---
    let (board_a, live_card, archived_card) = {
        let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
        let board_a = ctx.create_board("Alpha".into(), None)?;
        let col = ctx.create_column(board_a.id, "Todo".into(), None)?;
        let live = ctx.create_card(board_a.id, col.id, "Live".into(), Default::default())?;
        let arch = ctx.create_card(board_a.id, col.id, "ToArchive".into(), Default::default())?;
        ctx.archive_card(arch.id)?;

        // A second live board to prove board data survives the migration.
        ctx.create_board("Beta".into(), None)?;
        ctx.save().await?;
        (board_a.id, live.id, arch.id)
    };

    // --- Downgrade the saved file to a legacy V7 shape. ---
    downgrade_to_v7(&path);
    let v7_raw = std::fs::read_to_string(&path).unwrap();
    assert!(
        v7_raw.contains("\"version\":7"),
        "fixture must be version 7"
    );
    assert!(
        !v7_raw.contains("archived_boards"),
        "V7 fixture must not carry archived_boards"
    );

    // --- Reopen: loading triggers the V7 -> V8 migration chain. ---
    let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;

    // Boards survived (2 live boards).
    assert_eq!(ctx.boards()?.len(), 2, "both boards survived migration");
    // Live card survived; archived card is not in the live set.
    let live = ctx.list_all_cards()?;
    assert!(live.iter().any(|c| c.id == live_card), "live card survived");
    assert!(!live.iter().any(|c| c.id == archived_card));
    // Archived card survived AND its board_id was backfilled by V7->V8.
    let archived = ctx.list_archived_cards()?;
    assert_eq!(archived.len(), 1, "archived card survived migration");
    assert_eq!(archived[0].entity_id, archived_card);
    assert_eq!(
        archived[0].context.board_id, board_a,
        "V7->V8 backfilled archived_cards.board_id"
    );
    // archived_boards defaulted to empty (the V7 file had no such key).
    assert!(
        ctx.list_archived_boards()?.is_empty(),
        "archived_boards defaults empty on a migrated legacy file"
    );

    // --- The migrated store is fully functional: archive a board, save,
    //     reload, and confirm it round-trips (C4 path on migrated data). ---
    ctx.archive_board(board_a)?;
    assert_eq!(ctx.boards()?.len(), 1, "Alpha left the live set");
    assert_eq!(ctx.list_archived_boards()?.len(), 1);
    ctx.save().await?;

    let reloaded = JsonDataStore::new(Arc::new(JsonFileStore::new(&path)));
    assert_eq!(reloaded.list_boards()?.len(), 1);
    let ab = reloaded.list_archived_boards()?;
    assert_eq!(
        ab.len(),
        1,
        "archived board persisted after migrating a V7 file"
    );
    assert_eq!(ab[0].entity_id, board_a);
    // The originally-archived card is still archived on the reloaded store.
    assert_eq!(reloaded.list_archived_cards()?.len(), 1);
    Ok(())
}
