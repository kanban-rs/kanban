//! Integration tests for the `KanbanOperations` default resolver methods
//! (`resolve_board_id`, `resolve_column_id`, `resolve_sprint_id`, `resolve_card_id`,
//! and their `_global` / batch variants). Covered:
//!   - UUID fast path
//!   - name fast path (case-insensitive)
//!   - sprint number fast path
//!   - miss → message lists alternatives
//!   - ambiguity → message lists conflicts
//!   - batch resolvers aggregate failures
//!   - `require_same_board` accepts homogeneous batches and rejects cross-board ones

use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

async fn open_ctx() -> (KanbanContext, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    (ctx, dir)
}

// ---------- resolve_board_id ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_board_id_by_name_case_insensitive() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx
        .create_board("Kanban".into(), Some("KAN".into()))
        .unwrap();
    assert_eq!(ctx.resolve_board_id("Kanban").unwrap(), board.id);
    assert_eq!(ctx.resolve_board_id("kanban").unwrap(), board.id);
    assert_eq!(ctx.resolve_board_id("KANBAN").unwrap(), board.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_board_id_by_uuid_fast_path() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx
        .create_board("Kanban".into(), Some("KAN".into()))
        .unwrap();
    assert_eq!(
        ctx.resolve_board_id(&board.id.to_string()).unwrap(),
        board.id
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_board_id_not_found_lists_available() {
    let (mut ctx, _dir) = open_ctx().await;
    ctx.create_board("Kanban".into(), Some("KAN".into()))
        .unwrap();
    ctx.create_board("Personal".into(), Some("PER".into()))
        .unwrap();
    let err = ctx.resolve_board_id("missing").unwrap_err().to_string();
    assert!(err.contains("'missing' not found"), "got: {err}");
    assert!(err.contains("Kanban"), "got: {err}");
    assert!(err.contains("Personal"), "got: {err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_board_id_ambiguous_includes_label_and_uuid_per_match() {
    let (mut ctx, _dir) = open_ctx().await;
    let b1 = ctx.create_board("Shared".into(), None).unwrap();
    let b2 = ctx.create_board("Shared".into(), None).unwrap();
    let err = ctx.resolve_board_id("shared").unwrap_err().to_string();
    assert!(err.contains("ambiguous"), "got: {err}");
    // Every match now carries its UUID in the message — no need for the
    // "Specify by UUID" coda.
    assert!(err.contains(&b1.id.to_string()), "got: {err}");
    assert!(err.contains(&b2.id.to_string()), "got: {err}");
}

// ---------- resolve_column_id ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_column_id_by_name_within_board() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    assert_eq!(ctx.resolve_column_id("todo", board.id).unwrap(), col.id);
    assert_eq!(ctx.resolve_column_id("TODO", board.id).unwrap(), col.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_column_id_not_found_lists_columns_on_board() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    ctx.create_column(board.id, "TODO".into(), None).unwrap();
    ctx.create_column(board.id, "Doing".into(), None).unwrap();
    let err = ctx
        .resolve_column_id("nope", board.id)
        .unwrap_err()
        .to_string();
    assert!(err.contains("not found"), "got: {err}");
    assert!(err.contains("TODO"), "got: {err}");
    assert!(err.contains("Doing"), "got: {err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_column_id_global_finds_unique_across_boards() {
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    let col_a = ctx
        .create_column(board_a.id, "Backlog".into(), None)
        .unwrap();
    ctx.create_column(board_b.id, "Doing".into(), None).unwrap();
    assert_eq!(ctx.resolve_column_id_global("backlog").unwrap(), col_a.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_column_id_global_ambiguous_lists_board_names() {
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    ctx.create_column(board_a.id, "TODO".into(), None).unwrap();
    ctx.create_column(board_b.id, "TODO".into(), None).unwrap();
    let err = ctx
        .resolve_column_id_global("todo")
        .unwrap_err()
        .to_string();
    assert!(err.contains("ambiguous"), "got: {err}");
    assert!(err.contains("'A'"), "got: {err}");
    assert!(err.contains("'B'"), "got: {err}");
}

// ---------- resolve_sprint_id ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_id_by_name() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    let sprint = ctx
        .create_sprint(board.id, None, Some("yarara-release".into()))
        .unwrap();
    assert_eq!(
        ctx.resolve_sprint_id("yarara-release", board.id).unwrap(),
        sprint.id
    );
    assert_eq!(
        ctx.resolve_sprint_id("YARARA-RELEASE", board.id).unwrap(),
        sprint.id
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_id_by_number() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), None).unwrap();
    let sprint = ctx
        .create_sprint(board.id, None, Some("alpha".into()))
        .unwrap();
    let n = sprint.sprint_number;
    assert_eq!(
        ctx.resolve_sprint_id(&n.to_string(), board.id).unwrap(),
        sprint.id
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_id_global_finds_unique() {
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    let _s_a = ctx
        .create_sprint(board_a.id, None, Some("alpha".into()))
        .unwrap();
    let s_b = ctx
        .create_sprint(board_b.id, None, Some("beta".into()))
        .unwrap();
    assert_eq!(ctx.resolve_sprint_id_global("beta").unwrap(), s_b.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_id_global_ambiguous_number_lists_boards() {
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    let _ = ctx.create_sprint(board_a.id, None, None).unwrap();
    let _ = ctx.create_sprint(board_b.id, None, None).unwrap();
    let err = ctx.resolve_sprint_id_global("1").unwrap_err().to_string();
    assert!(err.contains("ambiguous"), "got: {err}");
    assert!(err.contains("'A'"), "got: {err}");
    assert!(err.contains("'B'"), "got: {err}");
}

// ---------- resolve_card_id / resolve_card_ids ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_id_by_identifier() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "Hello".into(), Default::default())
        .unwrap();
    let ident = format!("KAN-{}", card.card_number);
    assert_eq!(ctx.resolve_card_id(&ident).unwrap(), card.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_id_not_found_returns_not_found_by_name() {
    let (ctx, _dir) = open_ctx().await;
    let err = ctx.resolve_card_id("KAN-999").unwrap_err();
    assert!(err.is_not_found(), "got: {err}");
    assert!(err.is_not_found_by_name(), "got: {err}");
    assert!(err.to_string().contains("'KAN-999' not found"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_aggregates_failures_as_typed_variant() {
    use kanban_domain::{BatchResolutionCause, DomainError, KanbanError};
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(board.id, col.id, "ok".into(), Default::default())
        .unwrap();
    let ident_ok = format!("KAN-{}", card.card_number);
    let raws: Vec<String> = vec![ident_ok.clone(), "KAN-999".into(), "KAN-998".into()];

    let err = ctx.resolve_card_ids(&raws).unwrap_err();
    assert!(err.is_batch_resolution_failed(), "got: {err:?}");

    // Programmatic introspection: pull out the typed failures.
    let KanbanError::Domain(DomainError::BatchResolutionFailed { entity, failures }) = err else {
        panic!("expected BatchResolutionFailed");
    };
    assert_eq!(entity, "Card");
    assert_eq!(failures.len(), 2);
    let by_input: std::collections::HashMap<_, _> = failures
        .iter()
        .map(|f| (f.raw_input.clone(), &f.cause))
        .collect();
    assert!(matches!(
        by_input["KAN-999"],
        BatchResolutionCause::NotFound
    ));
    assert!(matches!(
        by_input["KAN-998"],
        BatchResolutionCause::NotFound
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_failure_display_lists_each_input() {
    let (mut ctx, _dir) = open_ctx().await;
    ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let err = ctx
        .resolve_card_ids(&["KAN-999".into(), "KAN-998".into()])
        .unwrap_err();
    let msg = err.to_string();
    // Entity stays capitalized, plural agrees with count (no "(s)" parenthetical).
    assert!(msg.contains("2 Cards"), "got: {msg}");
    assert!(msg.contains("'KAN-999'"), "got: {msg}");
    assert!(msg.contains("'KAN-998'"), "got: {msg}");
    assert!(msg.contains("not found"), "got: {msg}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_success_returns_all_uuids() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let c1 = ctx
        .create_card(board.id, col.id, "one".into(), Default::default())
        .unwrap();
    let c2 = ctx
        .create_card(board.id, col.id, "two".into(), Default::default())
        .unwrap();
    let raws: Vec<String> = vec![format!("KAN-{}", c1.card_number), c2.id.to_string()];
    let ids = ctx.resolve_card_ids(&raws).unwrap();
    assert_eq!(ids, vec![c1.id, c2.id]);
}

// ---------- require_same_board ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_require_same_board_accepts_homogeneous_batch() {
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let c1 = ctx
        .create_card(board.id, col.id, "one".into(), Default::default())
        .unwrap();
    let c2 = ctx
        .create_card(board.id, col.id, "two".into(), Default::default())
        .unwrap();
    let shared = ctx.require_same_board(&[c1.id, c2.id]).unwrap();
    assert_eq!(shared, board.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_require_same_board_rejects_cross_board_batch() {
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx
        .create_board("Kanban".into(), Some("KAN".into()))
        .unwrap();
    let board_b = ctx
        .create_board("Personal".into(), Some("PER".into()))
        .unwrap();
    let col_a = ctx.create_column(board_a.id, "TODO".into(), None).unwrap();
    let col_b = ctx.create_column(board_b.id, "TODO".into(), None).unwrap();
    let c_a = ctx
        .create_card(board_a.id, col_a.id, "a".into(), Default::default())
        .unwrap();
    let c_b = ctx
        .create_card(board_b.id, col_b.id, "b".into(), Default::default())
        .unwrap();
    let err = ctx
        .require_same_board(&[c_a.id, c_b.id])
        .unwrap_err()
        .to_string();
    assert!(err.contains("same board"), "got: {err}");
    assert!(err.contains("'Kanban'"), "got: {err}");
    assert!(err.contains("'Personal'"), "got: {err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_require_same_board_empty_batch_errors() {
    let (ctx, _dir) = open_ctx().await;
    let err = ctx.require_same_board(&[]).unwrap_err().to_string();
    assert!(err.contains("No cards"), "got: {err}");
}

// ---------- UUID fast path doesn't accidentally do lookup ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_uuid_fast_path_returns_uuid_even_when_nothing_exists() {
    let (ctx, _dir) = open_ctx().await;
    let random = Uuid::new_v4();
    // No entities exist; resolver returns the UUID; downstream get_* would be NotFound.
    assert_eq!(ctx.resolve_board_id(&random.to_string()).unwrap(), random);
}

// ---------- New error variants & API ergonomics (KAN-400 review fixes) ----------

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_board_miss_returns_not_found_by_name_variant() {
    let (mut ctx, _dir) = open_ctx().await;
    ctx.create_board("Kanban".into(), None).unwrap();
    let err = ctx.resolve_board_id("missing").unwrap_err();
    assert!(err.is_not_found_by_name(), "got: {err:?}");
    assert!(err.is_not_found(), "umbrella predicate also true");
    assert!(
        !err.is_validation(),
        "no longer the catch-all Validation variant"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_board_ambiguous_returns_ambiguous_variant() {
    let (mut ctx, _dir) = open_ctx().await;
    ctx.create_board("Shared".into(), None).unwrap();
    ctx.create_board("Shared".into(), None).unwrap();
    let err = ctx.resolve_board_id("shared").unwrap_err();
    assert!(err.is_ambiguous(), "got: {err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_column_id_global_with_zero_boards_is_graceful() {
    // No boards, no columns — error message lists "" (empty available) but doesn't crash.
    let (ctx, _dir) = open_ctx().await;
    let err = ctx.resolve_column_id_global("foo").unwrap_err();
    assert!(err.is_not_found_by_name(), "got: {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("'foo'"), "msg: {msg}");
    // No "Available:" segment when the list is empty.
    assert!(!msg.contains("Available:"), "msg: {msg}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_id_global_named_ambiguity_across_boards() {
    // Two boards, each with a sprint named "alpha"; resolver should flag ambiguity
    // and name both boards in the matches list.
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx.create_board("Alpha-Board".into(), None).unwrap();
    let board_b = ctx.create_board("Beta-Board".into(), None).unwrap();
    ctx.create_sprint(board_a.id, None, Some("alpha".into()))
        .unwrap();
    ctx.create_sprint(board_b.id, None, Some("alpha".into()))
        .unwrap();
    let err = ctx.resolve_sprint_id_global("alpha").unwrap_err();
    assert!(err.is_ambiguous(), "got: {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("'Alpha-Board'"), "msg: {msg}");
    assert!(msg.contains("'Beta-Board'"), "msg: {msg}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_id_on_board_does_not_bleed_other_board_numbers() {
    // KAN-400 footgun-regression: with the split API the on-board variant
    // never matches a sprint from another board, even with the same sprint_number.
    let (mut ctx, _dir) = open_ctx().await;
    let board_a = ctx.create_board("A".into(), None).unwrap();
    let board_b = ctx.create_board("B".into(), None).unwrap();
    let _s_a = ctx.create_sprint(board_a.id, None, None).unwrap();
    let s_b = ctx.create_sprint(board_b.id, None, None).unwrap();
    assert_eq!(s_b.sprint_number, 1, "both sprints are #1 by construction");
    // Resolving "1" on board B returns the B sprint, not A's.
    let resolved = ctx.resolve_sprint_id("1", board_b.id).unwrap();
    assert_eq!(resolved, s_b.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_mixed_uuid_identifier_and_number() {
    // Single batch with three input shapes: UUID, KAN-N identifier, bare number.
    // All resolve against one snapshot.
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let c1 = ctx
        .create_card(board.id, col.id, "one".into(), Default::default())
        .unwrap();
    let c2 = ctx
        .create_card(board.id, col.id, "two".into(), Default::default())
        .unwrap();
    let c3 = ctx
        .create_card(board.id, col.id, "three".into(), Default::default())
        .unwrap();

    let raws: Vec<String> = vec![
        c1.id.to_string(),                 // raw UUID
        format!("KAN-{}", c2.card_number), // prefixed identifier
        c3.card_number.to_string(),        // bare number
    ];
    let ids = ctx.resolve_card_ids(&raws).unwrap();
    assert_eq!(ids, vec![c1.id, c2.id, c3.id]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_takes_single_snapshot_per_batch() {
    // Behavioural check: even a batch of 10 inputs against 50 cards completes
    // without per-element backend churn. We assert correctness here; the perf
    // contract is provable by inspecting the implementation (one set of
    // list_all_* at the top, then pure in-memory matching).
    let (mut ctx, _dir) = open_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let mut all_ids = Vec::new();
    for i in 0..50 {
        let c = ctx
            .create_card(board.id, col.id, format!("c{i}"), Default::default())
            .unwrap();
        all_ids.push(c.id);
    }
    // Resolve the first 10 by identifier.
    let raws: Vec<String> = (1..=10).map(|n| format!("KAN-{n}")).collect();
    let resolved = ctx.resolve_card_ids(&raws).unwrap();
    assert_eq!(resolved.len(), 10);
    assert_eq!(resolved, all_ids[0..10]);
}
