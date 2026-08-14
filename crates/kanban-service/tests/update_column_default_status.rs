//! Service-tier integration tests for `KanbanContext::update_column`'s
//! `default_status` field: `ColumnUpdate.default_status` is
//! `Option<Option<CardStatus>>`, so set, clear, and leave-unchanged must be
//! three distinguishable outcomes.
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{
    AppConfig, CardStatus, ColumnUpdate, FieldUpdate, KanbanBackend, KanbanContext,
    KanbanOperations, NewColumn,
};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

fn ctx_for(name: &str) -> (tempfile::TempDir, KanbanContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join(format!("{name}.json"));
    let ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());
    (dir, ctx)
}

fn board_with_column(ctx: &mut KanbanContext, default_status: Option<CardStatus>) -> Uuid {
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    ctx.create_column_from_spec(
        None,
        NewColumn {
            board_id,
            name: "Doing".to_string(),
            wip_limit: None,
            default_status,
        },
    )
    .unwrap()
    .id
}

fn no_op_update() -> ColumnUpdate {
    ColumnUpdate {
        name: None,
        position: None,
        wip_limit: FieldUpdate::NoChange,
        default_status: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_column_sets_default_status() {
    let (_dir, mut ctx) = ctx_for("set");
    let column_id = board_with_column(&mut ctx, None);

    let updated = ctx
        .update_column(
            column_id,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::InProgress)),
                ..no_op_update()
            },
        )
        .unwrap();

    assert_eq!(updated.default_status, Some(CardStatus::InProgress));
    assert_eq!(
        ctx.get_column(column_id).unwrap().unwrap().default_status,
        Some(CardStatus::InProgress)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_column_clears_default_status() {
    let (_dir, mut ctx) = ctx_for("clear");
    let column_id = board_with_column(&mut ctx, Some(CardStatus::InProgress));

    let updated = ctx
        .update_column(
            column_id,
            ColumnUpdate {
                default_status: Some(None),
                ..no_op_update()
            },
        )
        .unwrap();

    assert_eq!(updated.default_status, None);
    assert_eq!(
        ctx.get_column(column_id).unwrap().unwrap().default_status,
        None
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_column_without_default_status_field_leaves_it_unchanged() {
    let (_dir, mut ctx) = ctx_for("unchanged");
    let column_id = board_with_column(&mut ctx, Some(CardStatus::InProgress));

    let updated = ctx
        .update_column(
            column_id,
            ColumnUpdate {
                name: Some("Renamed".to_string()),
                ..no_op_update()
            },
        )
        .unwrap();

    assert_eq!(updated.default_status, Some(CardStatus::InProgress));
    assert_eq!(
        ctx.get_column(column_id).unwrap().unwrap().default_status,
        Some(CardStatus::InProgress)
    );
}
