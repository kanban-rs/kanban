//! One spec for the prefix surface, run against every backend.
//!
//! A prefix row is a shared namespace holding the counters that allocate card
//! and sprint numbers for one name. Several boards may point at the same row,
//! so these tests never assume a prefix belongs to anyone.
//!
//! Every case round-trips through durable storage, because the failure mode
//! this surface exists to prevent is a counter that silently resets: an
//! allocator reading a zeroed counter re-mints numbers that existing cards
//! already carry.

use super::super::BackendFactory;
use kanban_domain::{DataStore, Prefix};
use tempfile::TempDir;

/// Writes through one backend instance, then reads through a fresh one over
/// the same path, so nothing can pass on cached state alone.
async fn reopened(
    factory: &BackendFactory,
    path: &std::path::Path,
    write: impl FnOnce(&dyn DataStore),
) -> std::sync::Arc<dyn crate::KanbanBackend> {
    let backend = factory(path);
    backend.reload().await.unwrap();
    write(backend.as_data_store());
    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(path);
    reopened.reload().await.unwrap();
    reopened
}

pub async fn test_prefix_upsert_and_get_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = reopened(factory, &path, |store| {
        store
            .upsert_prefix(Prefix {
                name: "kan".to_string(),
                card_counter: 12,
                sprint_counter: 7,
            })
            .unwrap();
    })
    .await;

    let prefix = backend
        .get_prefix("kan")
        .unwrap()
        .expect("the prefix must survive a save and reopen");
    assert_eq!(prefix.name, "kan");
    assert_eq!(prefix.card_counter, 12, "card counter must persist");
    assert_eq!(prefix.sprint_counter, 7, "sprint counter must persist");
}

pub async fn test_prefix_get_is_case_insensitive(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = reopened(factory, &path, |store| {
        store
            .upsert_prefix(Prefix {
                name: "kan".to_string(),
                card_counter: 3,
                sprint_counter: 0,
            })
            .unwrap();
    })
    .await;

    // The identifier resolver lowercases before matching, so a case-sensitive
    // lookup here would fork one namespace into two and let both allocate the
    // same number.
    for probe in ["kan", "KAN", "Kan"] {
        assert!(
            backend.get_prefix(probe).unwrap().is_some(),
            "get_prefix({probe:?}) must find the `kan` namespace"
        );
    }
}

pub async fn test_prefix_upsert_replaces_the_row_for_an_existing_name(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = reopened(factory, &path, |store| {
        store
            .upsert_prefix(Prefix {
                name: "kan".to_string(),
                card_counter: 1,
                sprint_counter: 0,
            })
            .unwrap();
        store
            .upsert_prefix(Prefix {
                name: "KAN".to_string(),
                card_counter: 9,
                sprint_counter: 2,
            })
            .unwrap();
    })
    .await;

    let all = backend.list_prefixes().unwrap();
    assert_eq!(
        all.len(),
        1,
        "`kan` and `KAN` are one namespace; a second row would let two owners \
         allocate the same number: {all:?}"
    );
    assert_eq!(all[0].card_counter, 9, "the later write wins");
    assert_eq!(all[0].sprint_counter, 2);
    assert_eq!(
        all[0].name, "kan",
        "`Prefix::name` is documented as always normalised, so the stored name \
         must not take the caller's casing. Backends disagreed here: SQLite's \
         ON CONFLICT leaves the name alone while the in-memory path replaced \
         the whole row, so the same two writes produced `kan` on one and `KAN` \
         on the other"
    );
}

pub async fn test_prefix_list_returns_every_namespace(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = reopened(factory, &path, |store| {
        for (name, card_counter) in [("kan", 1), ("dev", 2), ("task", 3)] {
            store
                .upsert_prefix(Prefix {
                    name: name.to_string(),
                    card_counter,
                    sprint_counter: 0,
                })
                .unwrap();
        }
    })
    .await;

    let mut names: Vec<String> = backend
        .list_prefixes()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["dev", "kan", "task"]);
}

pub async fn test_prefix_get_returns_none_for_an_unknown_name(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = reopened(factory, &path, |store| {
        store
            .upsert_prefix(Prefix {
                name: "kan".to_string(),
                card_counter: 1,
                sprint_counter: 0,
            })
            .unwrap();
    })
    .await;

    assert!(
        backend.get_prefix("nope").unwrap().is_none(),
        "an absent namespace must read as None, never as a zeroed row -- an \
         allocator cannot tell those apart and would start numbering from 1"
    );
}

/// The counters are what the whole prefix entity exists for. A backend that
/// stored the name but reset the counters would pass every test above that
/// only checks presence, and would then re-mint numbers existing cards carry.
pub async fn test_prefix_counters_survive_repeated_save_cycles(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = reopened(factory, &path, |store| {
        store
            .upsert_prefix(Prefix {
                name: "kan".to_string(),
                card_counter: 41,
                sprint_counter: 5,
            })
            .unwrap();
    })
    .await;
    backend.flush().await.unwrap();
    drop(backend);

    let backend = factory(&path);
    backend.reload().await.unwrap();
    let prefix = backend.get_prefix("kan").unwrap().expect("still present");
    assert_eq!(
        (prefix.card_counter, prefix.sprint_counter),
        (41, 5),
        "counters must survive a second save/reload cycle unchanged"
    );
}

/// A rejected create must not consume a card number, on EVERY backend.
///
/// This is a rollback-semantics test, and rollback is implemented per backend:
/// SQLite uses a real write transaction, while the JSON and in-memory backends
/// snapshot and restore. A snapshot that did not carry `prefixes` would restore
/// everything except the counter, so the number would be burned on those two
/// and not on SQLite -- passing a SQLite-only test while silently diverging.
pub async fn test_a_rejected_create_does_not_consume_a_card_number(factory: &BackendFactory) {
    use kanban_domain::{ColumnUpdate, CreateCardOptions, FieldUpdate, KanbanOperations};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = crate::KanbanContext::open(factory(&path), kanban_core::AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    ctx.update_column(
        col.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(1),
            ..Default::default()
        },
    )
    .unwrap();

    let first = ctx
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();
    let before = ctx
        .backend()
        .get_prefix("kan")
        .unwrap()
        .unwrap()
        .card_counter;

    let rejected = ctx.create_card(board.id, col.id, "two".into(), CreateCardOptions::default());
    assert!(rejected.is_err(), "the column is full, so this must fail");

    assert_eq!(
        ctx.backend()
            .get_prefix("kan")
            .unwrap()
            .unwrap()
            .card_counter,
        before,
        "the allocation is made inside the batch's transaction, so a command \
         that rejects must roll it back on this backend too"
    );

    ctx.delete_card(first.id).unwrap();
    let next = ctx
        .create_card(
            board.id,
            col.id,
            "three".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(
        next.card_number,
        first.card_number + 1,
        "and numbering stays contiguous"
    );
}
