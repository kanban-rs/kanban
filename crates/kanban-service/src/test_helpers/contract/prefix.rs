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
use kanban_domain::{Board, Card, Column, DataStore, Prefix, Snapshot};
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

fn seed_graph(prefix: &str, card_number: u32) -> (Snapshot, uuid::Uuid) {
    let board = Board::new("B", Some(prefix));
    let column = Column::new(board.id, "Todo", 0);
    let mut card = Card::new(board.id, column.id, "one", 0);
    card.prefix = prefix.to_string();
    card.card_number = card_number;
    let card_id = card.id;

    let mut snapshot = Snapshot::from_data(
        vec![board],
        vec![column],
        vec![card],
        Vec::new(),
        Vec::new(),
        Default::default(),
    );
    snapshot.prefixes = vec![Prefix {
        name: Prefix::normalize(prefix),
        card_counter: card_number,
        sprint_counter: 0,
    }];
    (snapshot, card_id)
}

fn snapshot_with_one_card(prefixes: Vec<Prefix>) -> Snapshot {
    let (mut snapshot, _) = seed_graph("KAN", 7);
    snapshot.prefixes = prefixes;
    snapshot
}

pub async fn test_a_whole_store_write_stores_prefix_rows_normalised(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let snapshot = snapshot_with_one_card(vec![Prefix {
        name: "KAN".to_string(),
        card_counter: 7,
        sprint_counter: 2,
    }]);

    let backend = factory(&path);
    backend.reload().await.unwrap();
    crate::store_adapter::write_full_snapshot(backend.as_data_store(), snapshot).unwrap();

    let assert_normalised = |backend: &std::sync::Arc<dyn crate::KanbanBackend>| {
        let all = backend.list_prefixes().unwrap();
        assert_eq!(all.len(), 1, "expected exactly one prefix row: {all:?}");
        assert_eq!(
            all[0].name, "kan",
            "`Prefix::name` is documented as always normalised, but apply_snapshot \
             stored the caller's casing verbatim: {all:?}"
        );
        assert_eq!(all[0].card_counter, 7);
        assert_eq!(all[0].sprint_counter, 2);
        assert!(backend.get_prefix("kan").unwrap().is_some());
        assert!(backend.get_prefix("KAN").unwrap().is_some());
    };

    assert_normalised(&backend);

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    assert_normalised(&reopened);
}

pub async fn test_a_whole_store_write_collapses_two_spellings_of_one_namespace(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let snapshot = snapshot_with_one_card(vec![
        Prefix {
            name: "kan".to_string(),
            card_counter: 1,
            sprint_counter: 0,
        },
        Prefix {
            name: "KAN".to_string(),
            card_counter: 9,
            sprint_counter: 2,
        },
    ]);

    let backend = factory(&path);
    backend.reload().await.unwrap();
    crate::store_adapter::write_full_snapshot(backend.as_data_store(), snapshot).unwrap();

    let assert_collapsed = |backend: &std::sync::Arc<dyn crate::KanbanBackend>| {
        let all = backend.list_prefixes().unwrap();
        assert_eq!(
            all.len(),
            1,
            "`kan` and `KAN` are one namespace; a second row would let two owners \
             allocate the same number: {all:?}"
        );
        assert_eq!(all[0].name, "kan");
        assert_eq!(all[0].card_counter, 9, "the later write wins");
        assert_eq!(all[0].sprint_counter, 2);
    };

    assert_collapsed(&backend);

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    assert_collapsed(&reopened);
}

/// A created card's namespace must be backed by a durable prefix row on
/// every backend, and the card must keep its own prefix across a reload.
pub async fn test_creating_a_card_leaves_its_namespace_backed(factory: &BackendFactory) {
    use kanban_domain::{CreateCardOptions, KanbanOperations};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = crate::KanbanContext::open(factory(&path), kanban_core::AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();

    ctx.backend().flush().await.unwrap();
    drop(ctx);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let prefix = reopened
        .get_prefix("kan")
        .unwrap()
        .expect("the namespace the card names must be backed by a durable row");
    assert!(
        prefix.card_counter >= card.card_number,
        "the row's counter must have advanced at least as far as the card it minted"
    );
    let reread = reopened
        .get_card(card.id)
        .unwrap()
        .expect("the card must survive the reload");
    assert_eq!(reread.prefix, card.prefix);
}

/// A subcard allocates its own number from its own namespace, independent
/// of any pre-allocation by the caller, and must be backed the same way.
pub async fn test_creating_a_subcard_leaves_its_namespace_backed(factory: &BackendFactory) {
    use kanban_domain::commands::{
        dependency_commands::CreateSubcardCommand, Command, DependencyCommand,
    };
    use kanban_domain::{CreateCardOptions, KanbanOperations};
    use uuid::Uuid;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = crate::KanbanContext::open(factory(&path), kanban_core::AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let parent = ctx
        .create_card(
            board.id,
            col.id,
            "parent".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let subcard_id = Uuid::new_v4();
    let _ = ctx
        .execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
            CreateSubcardCommand {
                id: subcard_id,
                parent_id: parent.id,
                board_id: board.id,
                column_id: col.id,
                title: "subcard".into(),
                description: None,
                position: 0,
                default_card_prefix: "task".into(),
            },
        ))])
        .unwrap();

    ctx.backend().flush().await.unwrap();
    drop(ctx);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let subcard = reopened
        .get_card(subcard_id)
        .unwrap()
        .expect("the subcard must survive the reload");
    let prefix = reopened
        .get_prefix("kan")
        .unwrap()
        .expect("the subcard's namespace must be backed by a durable row");
    assert!(prefix.card_counter >= subcard.card_number);
}

/// Restoring an archived card must not leave its namespace un-backed, and
/// must not disturb the counter the card's own number was minted from.
pub async fn test_restoring_an_archived_card_leaves_its_namespace_backed(factory: &BackendFactory) {
    use kanban_domain::{CreateCardOptions, KanbanOperations};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = crate::KanbanContext::open(factory(&path), kanban_core::AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();

    let counter_before = ctx
        .backend()
        .get_prefix("kan")
        .unwrap()
        .unwrap()
        .card_counter;

    ctx.archive_card(card.id).unwrap();
    let restored = ctx.restore_card(card.id, None).unwrap();

    ctx.backend().flush().await.unwrap();
    drop(ctx);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let prefix = reopened
        .get_prefix("kan")
        .unwrap()
        .expect("the restored card's namespace must still be backed by a durable row");
    assert_eq!(
        prefix.card_counter, counter_before,
        "restore must not mint or lose numbers from the namespace's counter"
    );
    let reread = reopened
        .get_card(restored.id)
        .unwrap()
        .expect("the restored card must survive the reload");
    assert_eq!(reread.prefix, card.prefix);
}

pub async fn test_a_whole_store_write_without_the_referenced_prefix_row_is_rejected_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let (seed, _card_id) = seed_graph("KAN", 7);
    let mut without_prefix = seed;
    without_prefix.prefixes.clear();

    let backend = factory(&path);
    backend.reload().await.unwrap();

    let store = backend.as_data_store();
    let err = backend
        .with_transaction(Box::new(move || {
            crate::store_adapter::write_full_snapshot(store, without_prefix)
        }))
        .unwrap_err();
    assert!(
        matches!(
            &err,
            kanban_domain::KanbanError::Domain(kanban_domain::DomainError::PrefixNotBacked {
                card_number: 7,
                prefix,
            }) if prefix == "KAN"
        ),
        "expected PrefixNotBacked for card 7 / KAN, got {err:?}"
    );

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    assert!(
        reopened.as_data_store().list_all_cards().unwrap().is_empty(),
        "the rejected write must not have reached durable storage"
    );
    assert!(
        reopened.list_prefixes().unwrap().is_empty(),
        "no prefix row should have been created for the rejected write"
    );
}

pub async fn test_a_whole_store_write_never_removes_a_namespace_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let (mut seed, _card_id) = seed_graph("KAN", 7);
    seed.prefixes.push(Prefix {
        name: "ops".to_string(),
        card_counter: 5,
        sprint_counter: 1,
    });

    let backend = factory(&path);
    backend.reload().await.unwrap();
    crate::store_adapter::write_full_snapshot(backend.as_data_store(), seed.clone()).unwrap();
    backend.flush().await.unwrap();
    drop(backend);

    let backend = factory(&path);
    backend.reload().await.unwrap();
    let mut empty_write = Snapshot::new();
    empty_write.prefixes.clear();
    crate::store_adapter::write_full_snapshot(backend.as_data_store(), empty_write).unwrap();
    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();

    let ops = reopened
        .get_prefix("ops")
        .unwrap()
        .expect("an unreferenced namespace must survive a whole-store write that omits it");
    assert_eq!(ops.card_counter, 5, "ops's counter must be untouched");
    assert_eq!(ops.sprint_counter, 1, "ops's counter must be untouched");

    let kan = reopened
        .get_prefix("kan")
        .unwrap()
        .expect("a referenced namespace must survive too");
    assert_eq!(kan.card_counter, 7);

    let cards = reopened.as_data_store().list_all_cards().unwrap();
    let card = cards
        .into_iter()
        .find(|c| c.card_number == 7)
        .expect("card 7 must still be present");
    assert_eq!(card.prefix, "KAN");
}

/// A namespace a live card still names must survive on every durable backend,
/// on every write path -- including `apply_snapshot`, not just a card upsert.
pub async fn test_a_referenced_namespace_cannot_be_removed_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let (mut seed, _card_id) = seed_graph("KAN", 7);
    seed.prefixes.push(Prefix {
        name: "ops".to_string(),
        card_counter: 0,
        sprint_counter: 0,
    });

    let backend = factory(&path);
    backend.reload().await.unwrap();
    backend
        .as_data_store()
        .apply_snapshot(seed.clone())
        .unwrap();
    backend.flush().await.unwrap();
    drop(backend);

    // Negative half: dropping the `kan` row while a card still names it must
    // be rejected, and the row must still be there after the rejected write.
    let mut without_kan = seed.clone();
    without_kan.prefixes.clear();

    let backend = factory(&path);
    backend.reload().await.unwrap();
    let err = backend
        .as_data_store()
        .apply_snapshot(without_kan)
        .unwrap_err();
    assert!(
        matches!(
            &err,
            kanban_domain::KanbanError::Domain(kanban_domain::DomainError::PrefixNotBacked {
                card_number: 7,
                prefix,
            }) if prefix == "KAN"
        ),
        "expected PrefixNotBacked for card 7 / KAN, got {err:?}"
    );
    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let prefix = reopened
        .get_prefix("kan")
        .unwrap()
        .expect("the `kan` row must survive a rejected apply_snapshot");
    assert_eq!(prefix.card_counter, 7);
    let cards = reopened.as_data_store().list_all_cards().unwrap();
    let card = cards
        .into_iter()
        .find(|c| c.card_number == 7)
        .expect("the card must still be present");
    assert_eq!(card.prefix, "KAN");
    drop(reopened);

    // Positive half: dropping the extra, unreferenced `ops` row must succeed.
    let mut without_ops = seed.clone();
    without_ops.prefixes.retain(|p| p.name != "ops");

    let backend = factory(&path);
    backend.reload().await.unwrap();
    backend.as_data_store().apply_snapshot(without_ops).unwrap();
    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let mut names: Vec<String> = reopened
        .list_prefixes()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["kan".to_string()]);
}

/// A card naming a namespace with no row must be rejected on the write path
/// itself, on every durable backend.
pub async fn test_an_unbacked_namespace_is_rejected_on_every_backend(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = factory(&path);
    backend.reload().await.unwrap();

    let board = Board::new("B", Some("ZZZ"));
    let column = Column::new(board.id, "Todo", 0);
    backend.as_data_store().upsert_board(board.clone()).unwrap();
    backend
        .as_data_store()
        .upsert_column(column.clone())
        .unwrap();

    let mut card = Card::new(board.id, column.id, "one", 0);
    card.prefix = "ZZZ".to_string();
    card.card_number = 4;

    let store = backend.as_data_store();
    let result = backend.with_transaction(Box::new(|| store.upsert_card(card.clone())));
    let err = result.unwrap_err();
    assert!(
        matches!(
            &err,
            kanban_domain::KanbanError::Domain(kanban_domain::DomainError::PrefixNotBacked {
                card_number: 4,
                prefix,
            }) if prefix == "ZZZ"
        ),
        "expected PrefixNotBacked for card 4 / ZZZ, got {err:?}"
    );

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    assert!(
        reopened
            .as_data_store()
            .list_all_cards()
            .unwrap()
            .is_empty(),
        "the rejected card must not have reached durable storage"
    );
    assert!(
        reopened.list_prefixes().unwrap().is_empty(),
        "no prefix row should have been created for the rejected card"
    );
}

/// A card whose casing differs from the row's stored, normalised name must
/// still be accepted, on every durable backend.
pub async fn test_configured_casing_is_backed_by_the_normalised_row_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = factory(&path);
    backend.reload().await.unwrap();

    backend
        .as_data_store()
        .upsert_prefix(Prefix::new("kan"))
        .unwrap();
    let board = Board::new("B", Some("KAN"));
    let column = Column::new(board.id, "Todo", 0);
    backend.as_data_store().upsert_board(board.clone()).unwrap();
    backend
        .as_data_store()
        .upsert_column(column.clone())
        .unwrap();

    let mut card = Card::new(board.id, column.id, "one", 0);
    card.prefix = "KAN".to_string();
    card.card_number = 3;

    let store = backend.as_data_store();
    backend
        .with_transaction(Box::new(|| store.upsert_card(card.clone())))
        .unwrap();

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let reread = reopened
        .get_card(card.id)
        .unwrap()
        .expect("the card must survive the reload");
    assert_eq!(
        reread.prefix, "KAN",
        "casing must be kept verbatim on the card"
    );
    assert!(reopened.get_prefix("kan").unwrap().is_some());
    assert!(reopened.get_prefix("KAN").unwrap().is_some());
    let all = reopened.list_prefixes().unwrap();
    assert_eq!(all.len(), 1, "expected exactly one prefix row: {all:?}");
    assert_eq!(all[0].name, "kan");
}

/// A NON-ASCII prefix must round-trip a card on every durable backend.
/// SQLite matches `cards.prefix_ref` against `prefixes.name` with `COLLATE
/// NOCASE`, which folds ASCII only, so the domain's normalisation must not
/// fold characters the storage layer cannot: a Unicode-lowercased row name
/// ("öst") fails the FK for a card stamped with the configured casing
/// ("ÖST"), rejecting the card outright on SQLite while JSON accepts it.
pub async fn test_non_ascii_prefix_round_trips_a_card_on_every_backend(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = factory(&path);
    backend.reload().await.unwrap();

    backend
        .as_data_store()
        .upsert_prefix(Prefix::new("ÖST"))
        .unwrap();
    let board = Board::new("B", Some("ÖST"));
    let column = Column::new(board.id, "Todo", 0);
    backend.as_data_store().upsert_board(board.clone()).unwrap();
    backend
        .as_data_store()
        .upsert_column(column.clone())
        .unwrap();

    let mut card = Card::new(board.id, column.id, "one", 0);
    card.prefix = "ÖST".to_string();
    card.card_number = 1;

    let store = backend.as_data_store();
    backend
        .with_transaction(Box::new(|| store.upsert_card(card.clone())))
        .expect("a card in a non-ASCII namespace must be storable");

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    let reread = reopened
        .get_card(card.id)
        .unwrap()
        .expect("the card must survive the reload");
    assert_eq!(reread.prefix, "ÖST");
    assert!(
        reopened.get_prefix("ÖST").unwrap().is_some(),
        "the row must be reachable by the configured spelling"
    );
    let all = reopened.list_prefixes().unwrap();
    assert_eq!(all.len(), 1, "expected exactly one prefix row: {all:?}");
}

/// A rejected write must leave every backend byte-identical, before AND after
/// a reload, so a failed batch cannot have partially reached disk.
pub async fn test_a_rejected_write_leaves_every_backend_unchanged(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let backend = factory(&path);
    backend.reload().await.unwrap();

    backend
        .as_data_store()
        .upsert_prefix(Prefix {
            name: "kan".to_string(),
            card_counter: 1,
            sprint_counter: 0,
        })
        .unwrap();
    let board = Board::new("B", Some("KAN"));
    let column = Column::new(board.id, "Todo", 0);
    backend.as_data_store().upsert_board(board.clone()).unwrap();
    backend
        .as_data_store()
        .upsert_column(column.clone())
        .unwrap();

    let mut good_card = Card::new(board.id, column.id, "good", 0);
    good_card.prefix = "KAN".to_string();
    good_card.card_number = 1;
    {
        let store = backend.as_data_store();
        let card = good_card.clone();
        backend
            .with_transaction(Box::new(|| store.upsert_card(card)))
            .unwrap();
    }
    backend.flush().await.unwrap();

    let capture = |backend: &std::sync::Arc<dyn crate::KanbanBackend>| {
        let store = backend.as_data_store();
        let mut boards = store.list_boards().unwrap();
        boards.sort_by_key(|b| b.id);
        let mut columns = store.list_all_columns().unwrap();
        columns.sort_by_key(|c| c.id);
        let mut cards = store.list_all_cards().unwrap();
        cards.sort_by_key(|c| c.id);
        let mut prefixes = store.list_prefixes().unwrap();
        prefixes.sort_by(|a, b| a.name.cmp(&b.name));
        (boards, columns, cards, prefixes)
    };

    let before = capture(&backend);

    let mut second_card = Card::new(board.id, column.id, "second", 0);
    second_card.prefix = "KAN".to_string();
    second_card.card_number = 2;
    let mut bad_card = Card::new(board.id, column.id, "bad", 0);
    bad_card.prefix = "ZZZ".to_string();
    bad_card.card_number = 9;

    let store = backend.as_data_store();
    let second = second_card.clone();
    let bad = bad_card.clone();
    let result = backend.with_transaction(Box::new(move || {
        store.upsert_card(second)?;
        store.upsert_card(bad)
    }));
    assert!(result.is_err(), "the transaction must fail");

    assert_eq!(
        capture(&backend),
        before,
        "a rejected batch must leave every collection unchanged before reload"
    );

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = factory(&path);
    reopened.reload().await.unwrap();
    assert_eq!(
        capture(&reopened),
        before,
        "a rejected batch must not have reached disk"
    );
}
