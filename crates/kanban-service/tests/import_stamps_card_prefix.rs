//! Import must stamp the prefix on a card that carries none.
//!
//! Files written before cards stored their prefix leave it empty, and nothing
//! downstream ever fills it in: the indexed identifier lookup keys on the
//! stored value, so such a card is unreachable by name, and every consumer that
//! needs a namespace has to guess one. Stamping at the seam removes the guess.
//!
//! The value stamped must be the one the JSON V15 -> V16 migration would have
//! written for the same file, or opening a legacy file and importing it would
//! disagree about what its cards are called.

use kanban_domain::{CreateCardOptions, KanbanOperations, KanbanResult};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

async fn open_json(path: &std::path::Path, default_card_prefix: Option<&str>) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let config = AppConfig {
        default_card_prefix: default_card_prefix.map(Into::into),
        ..Default::default()
    };
    KanbanContext::open(backend, config).await.unwrap()
}

/// Strips what a release predating stored card prefixes could not have written.
fn as_pre_prefix_release_export(exported: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(exported).unwrap();
    value["prefixes"] = serde_json::json!([]);
    for card in value["cards"].as_array_mut().unwrap() {
        card.as_object_mut().unwrap().remove("prefix");
    }
    serde_json::to_string(&value).unwrap()
}

async fn seed_export(ctx: &mut KanbanContext, board_prefix: Option<&str>) -> KanbanResult<String> {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let board = ctx.create_board("Src".into(), None)?;
    if let Some(p) = board_prefix {
        ctx.update_board(
            board.id,
            BoardUpdate {
                card_prefix: FieldUpdate::Set(p.to_string()),
                ..Default::default()
            },
        )?;
    }
    let column = ctx.create_column(board.id, "TODO".into(), None)?;
    for i in 0..3 {
        ctx.create_card(
            board.id,
            column.id,
            format!("card {i}"),
            CreateCardOptions::default(),
        )?;
    }
    ctx.export_board(Some(board.id))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_stamps_the_prefix_on_a_card_that_carries_none() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json"), None).await;
    let mut dest = open_json(&dir.path().join("dest.json"), None).await;

    let legacy = as_pre_prefix_release_export(&seed_export(&mut src, Some("KAN")).await.unwrap());
    dest.import_board(&legacy).unwrap();

    let cards = dest.list_all_cards().unwrap();
    assert_eq!(cards.len(), 3);
    for card in &cards {
        assert_eq!(
            card.prefix, "KAN",
            "an imported card was left with no stored prefix"
        );
    }
}

/// A stamped card is reachable by the identifier it displays. The indexed
/// lookup keys on the stored prefix, so an unstamped card matches nothing.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_stamped_card_is_findable_by_its_identifier() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json"), None).await;
    let mut dest = open_json(&dir.path().join("dest.json"), None).await;

    let legacy = as_pre_prefix_release_export(&seed_export(&mut src, Some("KAN")).await.unwrap());
    dest.import_board(&legacy).unwrap();

    let found = dest.find_cards_by_identifier("KAN-2").unwrap();
    assert_eq!(
        found.len(),
        1,
        "an imported card could not be found by name"
    );
    assert_eq!(found[0].card_number, 2);
}

/// The stamp must not depend on the importing workspace's configuration. The
/// numbers in these files were minted before `default_card_prefix` reached the
/// card allocator at all, so config never named them and must not name them
/// now, or the same file would import differently in two workspaces.
#[tokio::test(flavor = "multi_thread")]
async fn test_the_stamp_does_not_depend_on_the_importing_workspaces_config() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json"), None).await;
    let legacy = as_pre_prefix_release_export(&seed_export(&mut src, None).await.unwrap());

    let mut plain = open_json(&dir.path().join("plain.json"), None).await;
    let mut configured = open_json(&dir.path().join("configured.json"), Some("feat")).await;

    plain.import_board(&legacy).unwrap();
    configured.import_board(&legacy).unwrap();

    let plain_prefixes: Vec<String> = plain
        .list_all_cards()
        .unwrap()
        .iter()
        .map(|c| c.prefix.clone())
        .collect();
    let configured_prefixes: Vec<String> = configured
        .list_all_cards()
        .unwrap()
        .iter()
        .map(|c| c.prefix.clone())
        .collect();

    assert_eq!(
        plain_prefixes, configured_prefixes,
        "the same file was stamped differently in two workspaces"
    );
    assert!(plain_prefixes.iter().all(|p| p == "task"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_leaves_a_card_that_already_carries_a_prefix_untouched() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json"), None).await;
    let mut dest = open_json(&dir.path().join("dest.json"), None).await;

    let exported = seed_export(&mut src, Some("KAN")).await.unwrap();
    dest.import_board(&exported).unwrap();

    for card in dest.list_all_cards().unwrap() {
        assert_eq!(card.prefix, "KAN");
    }
}

/// Stamping happens before reconstruction, so the counter lands on the
/// namespace the cards now carry rather than on a guess.
#[tokio::test(flavor = "multi_thread")]
async fn test_a_stamped_import_raises_the_counter_of_the_namespace_it_stamped() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json"), None).await;
    let mut dest = open_json(&dir.path().join("dest.json"), Some("feat")).await;

    let legacy = as_pre_prefix_release_export(&seed_export(&mut src, None).await.unwrap());
    dest.import_board(&legacy).unwrap();

    let counter = |name: &str| {
        dest.backend()
            .get_prefix(name)
            .unwrap()
            .map_or(0, |p| p.card_counter)
    };
    assert_eq!(counter("task"), 3, "the stamped namespace was left at zero");
    assert_eq!(counter("feat"), 0);
}
