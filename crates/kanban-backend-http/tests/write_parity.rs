use kanban_backend_http::HttpBackend;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, BoardUpdate, Card, CardPriority, CardStatus, CardUpdate,
    Column, ColumnUpdate, CreateCardOptions, FieldUpdate, GraphOperations, Invalidation,
    KanbanOperations, KanbanResult, NewBoard, NewCard, NewColumn, Prefix, RelatesKind, Severity,
    SortField, SortOrder, Sprint, SprintUpdate, TaskListView,
};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, BatchOperationResult, KanbanContext};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Json,
    Sqlite,
}

async fn open_local(kind: Backend, path: &Path) -> KanbanContext {
    match kind {
        Backend::Json => {
            let backend: Arc<dyn kanban_service::KanbanBackend> =
                Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
            KanbanContext::open(backend, AppConfig::default())
                .await
                .unwrap()
        }
        Backend::Sqlite => {
            let backend: Arc<dyn kanban_service::KanbanBackend> =
                Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
            KanbanContext::open(backend, AppConfig::default())
                .await
                .unwrap()
        }
    }
}

async fn start_server(kind: Backend, path: &Path) -> TestServer {
    match kind {
        Backend::Json => TestServer::start_on_json(path).await,
        Backend::Sqlite => TestServer::start_on_sqlite(path).await,
    }
}

fn copy_store(kind: Backend, from: &Path, to: &Path) {
    std::fs::copy(from, to).unwrap();
    if kind == Backend::Sqlite {
        for ext in ["-wal", "-shm"] {
            let src = Path::new(&format!("{}{ext}", from.display())).to_path_buf();
            if src.exists() {
                let dst = Path::new(&format!("{}{ext}", to.display())).to_path_buf();
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }
}

async fn ctx_over(server: &TestServer) -> KanbanContext {
    let backend = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn a_new_board(name: &str, prefix: Option<&str>) -> NewBoard {
    NewBoard {
        name: name.to_string(),
        description: Some("parity fixture".to_string()),
        sprint_prefix: Some("SPR".to_string()),
        card_prefix: prefix.map(str::to_string),
        task_sort_field: Some(SortField::Priority),
        task_sort_order: Some(SortOrder::Descending),
        sprint_duration_days: Some(14),
        task_list_view: Some(TaskListView::ColumnView),
    }
}

fn fixed_due() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2030-06-01T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn a_new_column(board_id: Uuid, name: &str) -> NewColumn {
    NewColumn {
        board_id,
        name: name.to_string(),
        wip_limit: Some(3),
        default_status: Some(CardStatus::Todo),
    }
}

fn a_new_card(column_id: Uuid, title: &str) -> NewCard {
    NewCard {
        column_id,
        title: title.to_string(),
        description: None,
        priority: CardPriority::High,
        due_date: None,
        points: Some(3),
        sprint_id: None,
    }
}

#[derive(Debug)]
struct GraphSnapshot {
    boards: Vec<Board>,
    columns: Vec<Column>,
    cards: Vec<Card>,
    archived_card_rows: Vec<Card>,
    sprints: Vec<Sprint>,
    archived_cards: Vec<ArchivedCard>,
    archived_boards: Vec<ArchivedBoard>,
    prefixes: Vec<Prefix>,
    spawns: Vec<(Uuid, Uuid, bool)>,
    blocks: Vec<(Uuid, Uuid, Severity, bool)>,
    relates: Vec<(Uuid, Uuid, RelatesKind, bool)>,
}

fn snapshot(ctx: &KanbanContext) -> GraphSnapshot {
    let ds = ctx.data_store();

    let mut boards = ds.list_boards().unwrap();
    boards.sort_by_key(|b| b.id);

    let mut columns = ds.list_all_columns().unwrap();
    columns.sort_by_key(|c| c.id);

    let mut cards = ds.list_all_cards().unwrap();
    cards.sort_by_key(|c| c.id);

    let mut archived_cards = ds.list_archived_cards().unwrap();
    archived_cards.sort_by_key(|a| a.entity_id);

    let mut archived_card_rows: Vec<Card> = archived_cards
        .iter()
        .map(|marker| {
            ds.get_card(marker.entity_id).unwrap().unwrap_or_else(|| {
                panic!(
                    "archived marker {} has no live card row; the reference-marker \
                     model requires the row to outlive the marker",
                    marker.entity_id
                )
            })
        })
        .collect();
    archived_card_rows.sort_by_key(|c| c.id);

    let mut archived_boards = ds.list_archived_boards().unwrap();
    archived_boards.sort_by_key(|a| a.entity_id);

    let mut sprints = ds.list_all_sprints().unwrap();
    sprints.sort_by_key(|s| s.id);

    let mut prefixes = ds.list_prefixes().unwrap();
    prefixes.sort_by(|a, b| a.name.cmp(&b.name));

    let graph = ds.get_graph().unwrap();
    let mut spawns: Vec<(Uuid, Uuid, bool)> = graph
        .spawns_edges()
        .iter()
        .map(|e| (e.base.source, e.base.target, e.base.archived_at.is_some()))
        .collect();
    spawns.sort();

    let mut blocks: Vec<(Uuid, Uuid, Severity, bool)> = graph
        .blocks_edges()
        .iter()
        .map(|e| {
            (
                e.base.source,
                e.base.target,
                e.severity,
                e.base.archived_at.is_some(),
            )
        })
        .collect();
    blocks.sort();

    let mut relates: Vec<(Uuid, Uuid, RelatesKind, bool)> = graph
        .relates_edges()
        .iter()
        .map(|e| {
            (
                e.base.source,
                e.base.target,
                e.kind,
                e.base.archived_at.is_some(),
            )
        })
        .collect();
    relates.sort_by_key(|(a, b, kind, archived)| (*a, *b, format!("{kind:?}"), *archived));

    GraphSnapshot {
        boards,
        columns,
        cards,
        archived_card_rows,
        sprints,
        archived_cards,
        archived_boards,
        prefixes,
        spawns,
        blocks,
        relates,
    }
}

fn epoch() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(0, 0).unwrap()
}

fn canonicalize_sprint(sp: &mut Sprint, epoch: chrono::DateTime<chrono::Utc>) {
    sp.created_at = epoch;
    sp.updated_at = epoch;
    sp.start_date = sp.start_date.map(|_| epoch);
    sp.end_date = sp.end_date.map(|_| epoch);
}

fn canonicalize(s: &mut GraphSnapshot) {
    let epoch = epoch();

    for b in s.boards.iter_mut() {
        b.created_at = epoch;
        b.updated_at = epoch;
    }
    for c in s.columns.iter_mut() {
        c.created_at = epoch;
        c.updated_at = epoch;
    }
    for c in s.cards.iter_mut().chain(s.archived_card_rows.iter_mut()) {
        canonicalize_card(c, epoch);
    }
    for sp in s.sprints.iter_mut() {
        canonicalize_sprint(sp, epoch);
    }
    for a in s.archived_cards.iter_mut() {
        a.metadata.archived_at = epoch;
    }
    for a in s.archived_boards.iter_mut() {
        a.metadata.archived_at = epoch;
    }
}

fn canonicalize_card(c: &mut Card, epoch: chrono::DateTime<chrono::Utc>) {
    c.created_at = epoch;
    c.updated_at = epoch;
    c.completed_at = c.completed_at.map(|_| epoch);
    for log in c.sprint_logs.iter_mut() {
        log.started_at = epoch;
        log.ended_at = log.ended_at.map(|_| epoch);
    }
}

fn assert_card_eq(label: &str, a: &Card, b: &Card) {
    let Card {
        id: id_a,
        column_id: column_id_a,
        board_id: board_id_a,
        title: title_a,
        description: description_a,
        priority: priority_a,
        status: status_a,
        position: position_a,
        due_date: due_date_a,
        points: points_a,
        card_number: card_number_a,
        prefix: prefix_a,
        sprint_id: sprint_id_a,
        created_at: created_at_a,
        updated_at: updated_at_a,
        completed_at: completed_at_a,
        sprint_logs: sprint_logs_a,
    } = a;
    let Card {
        id: id_b,
        column_id: column_id_b,
        board_id: board_id_b,
        title: title_b,
        description: description_b,
        priority: priority_b,
        status: status_b,
        position: position_b,
        due_date: due_date_b,
        points: points_b,
        card_number: card_number_b,
        prefix: prefix_b,
        sprint_id: sprint_id_b,
        created_at: created_at_b,
        updated_at: updated_at_b,
        completed_at: completed_at_b,
        sprint_logs: sprint_logs_b,
    } = b;
    assert_eq!(id_a, id_b, "{label} id");
    assert_eq!(column_id_a, column_id_b, "{label} column_id");
    assert_eq!(board_id_a, board_id_b, "{label} board_id");
    assert_eq!(title_a, title_b, "{label} title");
    assert_eq!(description_a, description_b, "{label} description");
    assert_eq!(priority_a, priority_b, "{label} priority");
    assert_eq!(status_a, status_b, "{label} status");
    assert_eq!(position_a, position_b, "{label} position");
    assert_eq!(due_date_a, due_date_b, "{label} due_date");
    assert_eq!(points_a, points_b, "{label} points");
    assert_eq!(card_number_a, card_number_b, "{label} card_number");
    assert_eq!(prefix_a, prefix_b, "{label} prefix");
    assert_eq!(sprint_id_a, sprint_id_b, "{label} sprint_id");
    assert_eq!(created_at_a, created_at_b, "{label} created_at");
    assert_eq!(updated_at_a, updated_at_b, "{label} updated_at");
    assert_eq!(completed_at_a, completed_at_b, "{label} completed_at");
    assert_eq!(sprint_logs_a, sprint_logs_b, "{label} sprint_logs");
}

fn assert_board_eq(label: &str, a: &Board, b: &Board) {
    let Board {
        id: id_a,
        name: name_a,
        description: description_a,
        sprint_prefix: sprint_prefix_a,
        card_prefix: card_prefix_a,
        task_sort_field: task_sort_field_a,
        task_sort_order: task_sort_order_a,
        sprint_duration_days: sprint_duration_days_a,
        sprint_names: sprint_names_a,
        sprint_name_used_count: sprint_name_used_count_a,
        next_sprint_number: next_sprint_number_a,
        active_sprint_id: active_sprint_id_a,
        task_list_view: task_list_view_a,
        position: position_a,
        created_at: created_at_a,
        updated_at: updated_at_a,
    } = a;
    let Board {
        id: id_b,
        name: name_b,
        description: description_b,
        sprint_prefix: sprint_prefix_b,
        card_prefix: card_prefix_b,
        task_sort_field: task_sort_field_b,
        task_sort_order: task_sort_order_b,
        sprint_duration_days: sprint_duration_days_b,
        sprint_names: sprint_names_b,
        sprint_name_used_count: sprint_name_used_count_b,
        next_sprint_number: next_sprint_number_b,
        active_sprint_id: active_sprint_id_b,
        task_list_view: task_list_view_b,
        position: position_b,
        created_at: created_at_b,
        updated_at: updated_at_b,
    } = b;
    assert_eq!(id_a, id_b, "{label} id");
    assert_eq!(name_a, name_b, "{label} name");
    assert_eq!(description_a, description_b, "{label} description");
    assert_eq!(sprint_prefix_a, sprint_prefix_b, "{label} sprint_prefix");
    assert_eq!(card_prefix_a, card_prefix_b, "{label} card_prefix");
    assert_eq!(
        task_sort_field_a, task_sort_field_b,
        "{label} task_sort_field"
    );
    assert_eq!(
        task_sort_order_a, task_sort_order_b,
        "{label} task_sort_order"
    );
    assert_eq!(
        sprint_duration_days_a, sprint_duration_days_b,
        "{label} sprint_duration_days"
    );
    assert_eq!(sprint_names_a, sprint_names_b, "{label} sprint_names");
    assert_eq!(
        sprint_name_used_count_a, sprint_name_used_count_b,
        "{label} sprint_name_used_count"
    );
    assert_eq!(
        next_sprint_number_a, next_sprint_number_b,
        "{label} next_sprint_number"
    );
    assert_eq!(
        active_sprint_id_a, active_sprint_id_b,
        "{label} active_sprint_id"
    );
    assert_eq!(task_list_view_a, task_list_view_b, "{label} task_list_view");
    assert_eq!(position_a, position_b, "{label} position");
    assert_eq!(created_at_a, created_at_b, "{label} created_at");
    assert_eq!(updated_at_a, updated_at_b, "{label} updated_at");
}

fn assert_column_eq(label: &str, a: &Column, b: &Column) {
    let Column {
        id: id_a,
        board_id: board_id_a,
        name: name_a,
        position: position_a,
        wip_limit: wip_limit_a,
        default_status: default_status_a,
        created_at: created_at_a,
        updated_at: updated_at_a,
    } = a;
    let Column {
        id: id_b,
        board_id: board_id_b,
        name: name_b,
        position: position_b,
        wip_limit: wip_limit_b,
        default_status: default_status_b,
        created_at: created_at_b,
        updated_at: updated_at_b,
    } = b;
    assert_eq!(id_a, id_b, "{label} id");
    assert_eq!(board_id_a, board_id_b, "{label} board_id");
    assert_eq!(name_a, name_b, "{label} name");
    assert_eq!(position_a, position_b, "{label} position");
    assert_eq!(wip_limit_a, wip_limit_b, "{label} wip_limit");
    assert_eq!(default_status_a, default_status_b, "{label} default_status");
    assert_eq!(created_at_a, created_at_b, "{label} created_at");
    assert_eq!(updated_at_a, updated_at_b, "{label} updated_at");
}

fn assert_sprint_eq(label: &str, a: &Sprint, b: &Sprint) {
    let Sprint {
        id: id_a,
        board_id: board_id_a,
        sprint_number: sprint_number_a,
        name_index: name_index_a,
        prefix: prefix_a,
        card_prefix: card_prefix_a,
        status: status_a,
        start_date: start_date_a,
        end_date: end_date_a,
        created_at: created_at_a,
        updated_at: updated_at_a,
    } = a;
    let Sprint {
        id: id_b,
        board_id: board_id_b,
        sprint_number: sprint_number_b,
        name_index: name_index_b,
        prefix: prefix_b,
        card_prefix: card_prefix_b,
        status: status_b,
        start_date: start_date_b,
        end_date: end_date_b,
        created_at: created_at_b,
        updated_at: updated_at_b,
    } = b;
    assert_eq!(id_a, id_b, "{label} id");
    assert_eq!(board_id_a, board_id_b, "{label} board_id");
    assert_eq!(sprint_number_a, sprint_number_b, "{label} sprint_number");
    assert_eq!(name_index_a, name_index_b, "{label} name_index");
    assert_eq!(prefix_a, prefix_b, "{label} prefix");
    assert_eq!(card_prefix_a, card_prefix_b, "{label} card_prefix");
    assert_eq!(status_a, status_b, "{label} status");
    assert_eq!(start_date_a, start_date_b, "{label} start_date");
    assert_eq!(end_date_a, end_date_b, "{label} end_date");
    assert_eq!(created_at_a, created_at_b, "{label} created_at");
    assert_eq!(updated_at_a, updated_at_b, "{label} updated_at");
}

fn assert_prefix_eq(label: &str, a: &Prefix, b: &Prefix) {
    let Prefix {
        name: name_a,
        card_counter: card_counter_a,
        sprint_counter: sprint_counter_a,
    } = a;
    let Prefix {
        name: name_b,
        card_counter: card_counter_b,
        sprint_counter: sprint_counter_b,
    } = b;
    assert_eq!(name_a, name_b, "{label} name");
    assert_eq!(card_counter_a, card_counter_b, "{label} card_counter");
    assert_eq!(sprint_counter_a, sprint_counter_b, "{label} sprint_counter");
}

fn assert_snapshot_eq(remote: &GraphSnapshot, control: &GraphSnapshot) {
    assert_eq!(remote.boards.len(), control.boards.len(), "boards.len");
    for (i, (a, b)) in remote.boards.iter().zip(control.boards.iter()).enumerate() {
        assert_board_eq(&format!("board[{i}]"), a, b);
    }

    assert_eq!(remote.columns.len(), control.columns.len(), "columns.len");
    for (i, (a, b)) in remote
        .columns
        .iter()
        .zip(control.columns.iter())
        .enumerate()
    {
        assert_column_eq(&format!("column[{i}]"), a, b);
    }

    assert_eq!(remote.cards.len(), control.cards.len(), "cards.len");
    for (i, (a, b)) in remote.cards.iter().zip(control.cards.iter()).enumerate() {
        assert_card_eq(&format!("card[{i}]"), a, b);
    }

    assert_eq!(
        remote.archived_card_rows.len(),
        control.archived_card_rows.len(),
        "archived_card_rows.len"
    );
    for (i, (a, b)) in remote
        .archived_card_rows
        .iter()
        .zip(control.archived_card_rows.iter())
        .enumerate()
    {
        assert_card_eq(&format!("archived_card_row[{i}]"), a, b);
    }

    assert_eq!(remote.sprints.len(), control.sprints.len(), "sprints.len");
    for (i, (a, b)) in remote
        .sprints
        .iter()
        .zip(control.sprints.iter())
        .enumerate()
    {
        assert_sprint_eq(&format!("sprint[{i}]"), a, b);
    }

    assert_eq!(
        remote.archived_cards.len(),
        control.archived_cards.len(),
        "archived_cards.len"
    );
    assert_eq!(
        remote.archived_cards, control.archived_cards,
        "archived_cards"
    );

    assert_eq!(
        remote.archived_boards.len(),
        control.archived_boards.len(),
        "archived_boards.len"
    );
    assert_eq!(
        remote.archived_boards, control.archived_boards,
        "archived_boards"
    );

    assert_eq!(
        remote.prefixes.len(),
        control.prefixes.len(),
        "prefixes.len"
    );
    for (i, (a, b)) in remote
        .prefixes
        .iter()
        .zip(control.prefixes.iter())
        .enumerate()
    {
        assert_prefix_eq(&format!("prefix[{i}]"), a, b);
    }

    assert_eq!(remote.spawns, control.spawns, "spawns edges");
    assert_eq!(remote.blocks, control.blocks, "blocks edges");
    assert_eq!(remote.relates, control.relates, "relates edges");
}

async fn lifecycle_parity(kind: Backend) {
    let dir = tempfile::tempdir().unwrap();
    let remote_path = dir.path().join("remote.store");
    let control_path = dir.path().join("control.store");

    let server = start_server(kind, &remote_path).await;
    let mut remote = ctx_over(&server).await;

    let board_id = Uuid::new_v4();
    let (board, _) = remote
        .create_board_from_spec(Some(board_id), a_new_board("Lifecycle", Some("LC")))
        .unwrap();

    let (column_a, _) = remote
        .create_column_from_spec(None, a_new_column(board.id, "Todo"))
        .unwrap();
    let (column_b, _) = remote
        .create_column_from_spec(None, a_new_column(board.id, "Doing"))
        .unwrap();
    let (column_c, _) = remote
        .create_column_from_spec(None, a_new_column(board.id, "Kept"))
        .unwrap();

    let card_a_id = Uuid::new_v4();
    let card_b_id = Uuid::new_v4();
    let card_c_id = Uuid::new_v4();
    let (card_a, _) = remote
        .create_card_from_spec(Some(card_a_id), a_new_card(column_a.id, "Card A"))
        .unwrap();
    let (card_b, _) = remote
        .create_card_from_spec(Some(card_b_id), a_new_card(column_a.id, "Card B"))
        .unwrap();
    let _ = remote
        .create_card_from_spec(
            Some(card_c_id),
            NewCard {
                due_date: Some(fixed_due()),
                ..a_new_card(column_c.id, "Card C")
            },
        )
        .unwrap();

    let _ = remote
        .update_board_impl(
            board.id,
            BoardUpdate {
                name: Some("Lifecycle Renamed".to_string()),
                description: FieldUpdate::Set("renamed via http".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let _ = remote
        .update_column_impl(
            column_a.id,
            ColumnUpdate {
                name: Some("Done".to_string()),
                position: None,
                wip_limit: FieldUpdate::Clear,
                default_status: Some(Some(CardStatus::Done)),
            },
        )
        .unwrap();

    let _ = remote
        .update_card_impl(
            card_a.id,
            CardUpdate {
                title: Some("Card A Updated".to_string()),
                status: Some(CardStatus::Done),
                description: FieldUpdate::Set("desc".to_string()),
                points: FieldUpdate::Set(5),
                due_date: FieldUpdate::Clear,
                ..Default::default()
            },
        )
        .unwrap();

    let _ = remote
        .update_card_impl(
            card_b.id,
            CardUpdate {
                column_id: Some(column_b.id),
                ..Default::default()
            },
        )
        .unwrap();
    let _ = remote.delete_card_impl(card_b.id).unwrap();
    let _ = remote.delete_column_impl(column_b.id).unwrap();

    drop(remote);

    let mut local = open_local(kind, &control_path).await;

    let (l_board, _) = local
        .create_board_from_spec(Some(board_id), a_new_board("Lifecycle", Some("LC")))
        .unwrap();
    let (l_column_a, _) = local
        .create_column_from_spec(Some(column_a.id), a_new_column(l_board.id, "Todo"))
        .unwrap();
    let (l_column_b, _) = local
        .create_column_from_spec(Some(column_b.id), a_new_column(l_board.id, "Doing"))
        .unwrap();
    let (l_column_c, _) = local
        .create_column_from_spec(Some(column_c.id), a_new_column(l_board.id, "Kept"))
        .unwrap();
    let (l_card_a, _) = local
        .create_card_from_spec(Some(card_a_id), a_new_card(l_column_a.id, "Card A"))
        .unwrap();
    let (l_card_b, _) = local
        .create_card_from_spec(Some(card_b_id), a_new_card(l_column_a.id, "Card B"))
        .unwrap();
    let _ = local
        .create_card_from_spec(
            Some(card_c_id),
            NewCard {
                due_date: Some(fixed_due()),
                ..a_new_card(l_column_c.id, "Card C")
            },
        )
        .unwrap();

    let _ = local
        .update_board_impl(
            l_board.id,
            BoardUpdate {
                name: Some("Lifecycle Renamed".to_string()),
                description: FieldUpdate::Set("renamed via http".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let _ = local
        .update_column_impl(
            l_column_a.id,
            ColumnUpdate {
                name: Some("Done".to_string()),
                position: None,
                wip_limit: FieldUpdate::Clear,
                default_status: Some(Some(CardStatus::Done)),
            },
        )
        .unwrap();

    let _ = local
        .update_card_impl(
            l_card_a.id,
            CardUpdate {
                title: Some("Card A Updated".to_string()),
                status: Some(CardStatus::Done),
                description: FieldUpdate::Set("desc".to_string()),
                points: FieldUpdate::Set(5),
                due_date: FieldUpdate::Clear,
                ..Default::default()
            },
        )
        .unwrap();

    let _ = local
        .update_card_impl(
            l_card_b.id,
            CardUpdate {
                column_id: Some(l_column_b.id),
                ..Default::default()
            },
        )
        .unwrap();
    let _ = local.delete_card_impl(l_card_b.id).unwrap();
    let _ = local.delete_column_impl(l_column_b.id).unwrap();
    local.save().await.unwrap();
    drop(local);

    server.shutdown().await;

    let remote_reopened = open_local(kind, &remote_path).await;
    let local_reopened = open_local(kind, &control_path).await;

    let mut remote_snap = snapshot(&remote_reopened);
    let mut control_snap = snapshot(&local_reopened);
    canonicalize(&mut remote_snap);
    canonicalize(&mut control_snap);

    assert_snapshot_eq(&remote_snap, &control_snap);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_nine_remote_ops_leave_store_graph_equal_to_local_json() {
    lifecycle_parity(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_nine_remote_ops_leave_store_graph_equal_to_local_sqlite() {
    lifecycle_parity(Backend::Sqlite).await;
}

async fn cascade_parity(kind: Backend) {
    let dir = tempfile::tempdir().unwrap();
    let remote_path = dir.path().join("remote.store");
    let control_path = dir.path().join("control.store");

    let mut seed_ctx = open_local(kind, &remote_path).await;
    let board_id = seed_ctx
        .create_board("Cascade".to_string(), Some("CAS".to_string()))
        .unwrap()
        .id;
    let column_id = seed_ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap()
        .id;
    let card_a = seed_ctx
        .create_card(
            board_id,
            column_id,
            "Card A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let card_b = seed_ctx
        .create_card(
            board_id,
            column_id,
            "Card B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let card_c = seed_ctx
        .create_card(
            board_id,
            column_id,
            "Card C".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let sprint_id = seed_ctx.create_sprint(board_id, None, None).unwrap().id;
    seed_ctx.assign_card_to_sprint(card_c, sprint_id).unwrap();
    seed_ctx.archive_card(card_b).unwrap();
    seed_ctx.attach_children(card_a, vec![card_c]).unwrap();
    seed_ctx.save().await.unwrap();
    drop(seed_ctx);

    copy_store(kind, &remote_path, &control_path);

    let server = start_server(kind, &remote_path).await;
    let mut remote = ctx_over(&server).await;
    let _ = remote.delete_board_impl(board_id).unwrap();
    drop(remote);
    server.shutdown().await;

    let mut local = open_local(kind, &control_path).await;
    let _ = local.delete_board_impl(board_id).unwrap();
    local.save().await.unwrap();
    drop(local);

    let remote_reopened = open_local(kind, &remote_path).await;
    let local_reopened = open_local(kind, &control_path).await;

    let mut remote_snap = snapshot(&remote_reopened);
    let mut control_snap = snapshot(&local_reopened);

    assert!(
        !remote_snap.prefixes.is_empty(),
        "seed must leave at least one prefix row so the cascade assertion is not vacuous"
    );

    canonicalize(&mut remote_snap);
    canonicalize(&mut control_snap);

    assert_snapshot_eq(&remote_snap, &control_snap);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_delete_board_cascade_matches_local_cascade_json() {
    cascade_parity(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_delete_board_cascade_matches_local_cascade_sqlite() {
    cascade_parity(Backend::Sqlite).await;
}

async fn numbering_parity(kind: Backend) {
    let dir = tempfile::tempdir().unwrap();
    let remote_path = dir.path().join("remote.store");
    let control_path = dir.path().join("control.store");

    let server = start_server(kind, &remote_path).await;
    let mut remote = ctx_over(&server).await;

    let board_a_id = Uuid::new_v4();
    let board_b_id = Uuid::new_v4();
    let (board_a, _) = remote
        .create_board_from_spec(Some(board_a_id), a_new_board("Shared A", Some("SHR")))
        .unwrap();
    let (board_b, _) = remote
        .create_board_from_spec(Some(board_b_id), a_new_board("Shared B", Some("SHR")))
        .unwrap();
    let (column_a, _) = remote
        .create_column_from_spec(None, a_new_column(board_a.id, "Todo"))
        .unwrap();
    let (column_b, _) = remote
        .create_column_from_spec(None, a_new_column(board_b.id, "Todo"))
        .unwrap();

    let card_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    let order = [
        (&column_a, card_ids[0]),
        (&column_b, card_ids[1]),
        (&column_a, card_ids[2]),
        (&column_b, card_ids[3]),
        (&column_a, card_ids[4]),
    ];
    let mut remote_cards = Vec::new();
    for (column, id) in &order {
        let (card, _) = remote
            .create_card_from_spec(Some(*id), a_new_card(column.id, "T"))
            .unwrap();
        remote_cards.push(card);
    }
    drop(remote);
    server.shutdown().await;

    let mut local = open_local(kind, &control_path).await;
    let (l_board_a, _) = local
        .create_board_from_spec(Some(board_a_id), a_new_board("Shared A", Some("SHR")))
        .unwrap();
    let (l_board_b, _) = local
        .create_board_from_spec(Some(board_b_id), a_new_board("Shared B", Some("SHR")))
        .unwrap();
    let (l_column_a, _) = local
        .create_column_from_spec(Some(column_a.id), a_new_column(l_board_a.id, "Todo"))
        .unwrap();
    let (l_column_b, _) = local
        .create_column_from_spec(Some(column_b.id), a_new_column(l_board_b.id, "Todo"))
        .unwrap();
    let l_order = [
        (&l_column_a, card_ids[0]),
        (&l_column_b, card_ids[1]),
        (&l_column_a, card_ids[2]),
        (&l_column_b, card_ids[3]),
        (&l_column_a, card_ids[4]),
    ];
    for (column, id) in &l_order {
        let _ = local
            .create_card_from_spec(Some(*id), a_new_card(column.id, "T"))
            .unwrap();
    }
    local.save().await.unwrap();
    drop(local);

    let remote_reopened = open_local(kind, &remote_path).await;
    let local_reopened = open_local(kind, &control_path).await;

    let mut remote_numbers: Vec<(String, u32)> = card_ids
        .iter()
        .map(|id| {
            let c = remote_reopened.data_store().get_card(*id).unwrap().unwrap();
            (c.prefix, c.card_number)
        })
        .collect();
    let mut control_numbers: Vec<(String, u32)> = card_ids
        .iter()
        .map(|id| {
            let c = local_reopened.data_store().get_card(*id).unwrap().unwrap();
            (c.prefix, c.card_number)
        })
        .collect();

    assert_eq!(
        remote_numbers, control_numbers,
        "per-card (prefix, card_number)"
    );

    remote_numbers.sort_by_key(|(_, n)| *n);
    let numbers: Vec<u32> = remote_numbers.iter().map(|(_, n)| *n).collect();
    assert_eq!(
        numbers,
        vec![1, 2, 3, 4, 5],
        "shared-namespace counter must allocate a contiguous sequence across both boards"
    );

    control_numbers.sort_by_key(|(_, n)| *n);
    let control_only_numbers: Vec<u32> = control_numbers.iter().map(|(_, n)| *n).collect();
    assert_eq!(control_only_numbers, vec![1, 2, 3, 4, 5]);

    let remote_prefix = remote_reopened
        .data_store()
        .get_prefix("shr")
        .unwrap()
        .expect("shr prefix row should exist remotely");
    let control_prefix = local_reopened
        .data_store()
        .get_prefix("shr")
        .unwrap()
        .expect("shr prefix row should exist locally");
    assert_eq!(
        remote_prefix.card_counter, control_prefix.card_counter,
        "shr prefix card_counter"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_create_card_numbering_matches_local_allocation_json() {
    numbering_parity(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_create_card_numbering_matches_local_allocation_sqlite() {
    numbering_parity(Backend::Sqlite).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_create_column_ignores_the_client_supplied_id() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx
        .create_board_from_spec(None, a_new_board("B", None))
        .unwrap();

    let chosen_uuid = Uuid::new_v4();
    let (column, _) = ctx
        .create_column_from_spec(Some(chosen_uuid), a_new_column(board.id, "Todo"))
        .unwrap();

    assert_ne!(column.id, chosen_uuid);

    server.shutdown().await;
}

struct Seeded {
    board_id: Uuid,
    todo: Uuid,
    done: Uuid,
    card_a: Uuid,
    card_b: Uuid,
    card_c: Uuid,
    card_d: Uuid,
    sprint_id: Uuid,
    other_board_id: Uuid,
    backlog: Uuid,
    card_e: Uuid,
}

fn seed_graph(ctx: &mut KanbanContext) -> Seeded {
    let board_id = ctx
        .create_board("Parity".to_string(), Some("PAR".to_string()))
        .unwrap()
        .id;
    let todo = ctx
        .create_column(board_id, "Todo".to_string(), None)
        .unwrap()
        .id;
    let done = ctx
        .create_column(board_id, "Done".to_string(), None)
        .unwrap()
        .id;
    let _ = ctx
        .update_column_impl(
            done,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::Done)),
                ..Default::default()
            },
        )
        .unwrap();

    let card_a = ctx
        .create_card(
            board_id,
            todo,
            "Card A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let card_b = ctx
        .create_card(
            board_id,
            todo,
            "Card B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let card_c = ctx
        .create_card(
            board_id,
            todo,
            "Card C".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;

    let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;
    ctx.assign_card_to_sprint(card_c, sprint_id).unwrap();
    ctx.archive_card(card_b).unwrap();
    ctx.attach_children(card_a, vec![card_c]).unwrap();

    let card_d = ctx
        .create_card(
            board_id,
            todo,
            "Card D".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let other_board_id = ctx
        .create_board("Other".to_string(), Some("OTH".to_string()))
        .unwrap()
        .id;
    let backlog = ctx
        .create_column(other_board_id, "Backlog".to_string(), None)
        .unwrap()
        .id;
    let card_e = ctx
        .create_card(
            other_board_id,
            backlog,
            "Card E".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    ctx.block(card_d, card_a, Severity::Medium).unwrap();
    ctx.relate(card_d, card_e, RelatesKind::MentionedIn)
        .unwrap();

    Seeded {
        board_id,
        todo,
        done,
        card_a,
        card_b,
        card_c,
        card_d,
        sprint_id,
        other_board_id,
        backlog,
        card_e,
    }
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Invalidated(Invalidation),
    Counted(usize, Invalidation),
    Card(Card, Invalidation),
    Sprint(Sprint, Invalidation),
    Batch {
        succeeded: Vec<Uuid>,
        failed: Vec<(Uuid, String)>,
        invalidation: Invalidation,
    },
    Rejected(String),
}

type Outcomes = Vec<(&'static str, Outcome)>;

impl Outcome {
    fn unit(r: KanbanResult<((), Invalidation)>) -> Self {
        r.map_or_else(
            |e| Self::Rejected(e.to_string()),
            |((), inv)| Self::Invalidated(inv),
        )
    }
    fn invalidated(r: KanbanResult<Invalidation>) -> Self {
        r.map_or_else(|e| Self::Rejected(e.to_string()), Self::Invalidated)
    }
    fn counted(r: KanbanResult<(usize, Invalidation)>) -> Self {
        r.map_or_else(
            |e| Self::Rejected(e.to_string()),
            |(n, inv)| Self::Counted(n, inv),
        )
    }
    fn card(r: KanbanResult<(Card, Invalidation)>) -> Self {
        r.map_or_else(
            |e| Self::Rejected(e.to_string()),
            |(mut card, inv)| {
                canonicalize_card(&mut card, epoch());
                card.sprint_logs.clear();
                Self::Card(card, inv)
            },
        )
    }
    fn sprint(r: KanbanResult<(Sprint, Invalidation)>) -> Self {
        r.map_or_else(
            |e| Self::Rejected(e.to_string()),
            |(mut sprint, inv)| {
                canonicalize_sprint(&mut sprint, epoch());
                Self::Sprint(sprint, inv)
            },
        )
    }
    fn batch((result, invalidation): (BatchOperationResult, Invalidation)) -> Self {
        Self::Batch {
            succeeded: result.succeeded,
            failed: result.failed.into_iter().map(|f| (f.id, f.error)).collect(),
            invalidation,
        }
    }
}

struct BothRuns {
    remote: Outcomes,
    local: Outcomes,
    remote_snap: GraphSnapshot,
    control_snap: GraphSnapshot,
}

async fn run_both<S>(
    kind: Backend,
    seed: impl FnOnce(&mut KanbanContext) -> S,
    op: impl Fn(&mut KanbanContext, &S) -> Outcomes,
) -> BothRuns {
    let dir = tempfile::tempdir().unwrap();
    let remote_path = dir.path().join("remote.store");
    let control_path = dir.path().join("control.store");

    let mut seed_ctx = open_local(kind, &remote_path).await;
    let seeded = seed(&mut seed_ctx);
    seed_ctx.save().await.unwrap();
    drop(seed_ctx);

    copy_store(kind, &remote_path, &control_path);

    let server = start_server(kind, &remote_path).await;
    let mut remote_ctx = ctx_over(&server).await;
    let remote = op(&mut remote_ctx, &seeded);
    drop(remote_ctx);
    server.shutdown().await;

    let mut local_ctx = open_local(kind, &control_path).await;
    let local = op(&mut local_ctx, &seeded);
    local_ctx.save().await.unwrap();
    drop(local_ctx);

    let remote_reopened = open_local(kind, &remote_path).await;
    let local_reopened = open_local(kind, &control_path).await;

    let mut remote_snap = snapshot(&remote_reopened);
    let mut control_snap = snapshot(&local_reopened);

    assert!(
        !remote_snap.prefixes.is_empty(),
        "seed must leave at least one prefix row so op_parity is not vacuous"
    );

    canonicalize(&mut remote_snap);
    canonicalize(&mut control_snap);

    BothRuns {
        remote,
        local,
        remote_snap,
        control_snap,
    }
}

async fn op_parity<S>(
    kind: Backend,
    seed: impl FnOnce(&mut KanbanContext) -> S,
    op: impl Fn(&mut KanbanContext, &S) -> Outcomes,
) -> BothRuns {
    let runs = run_both(kind, seed, op).await;
    assert_snapshot_eq(&runs.remote_snap, &runs.control_snap);
    assert_no_rejections("remote", &runs.remote);
    assert_no_rejections("local", &runs.local);
    assert_outcomes_eq(&runs.remote, &runs.local);
    runs
}

fn assert_no_rejections(side: &str, outcomes: &Outcomes) {
    for (label, outcome) in outcomes {
        assert!(
            !matches!(outcome, Outcome::Rejected(_)),
            "{side} op `{label}` was rejected: {outcome:?}"
        );
    }
}

fn assert_outcomes_eq(remote: &Outcomes, local: &Outcomes) {
    let labels = |o: &Outcomes| o.iter().map(|(l, _)| *l).collect::<Vec<_>>();
    assert_eq!(labels(remote), labels(local), "op outcome labels");
    for ((label, r), (_, l)) in remote.iter().zip(local) {
        assert_eq!(r, l, "op outcome `{label}` differs between HTTP and local");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = "differs between HTTP and local")]
async fn test_op_parity_panics_when_remote_and_local_outcomes_differ() {
    let counter = std::cell::Cell::new(0usize);
    op_parity(Backend::Json, seed_graph, |_ctx, _s| {
        let n = counter.get();
        counter.set(n + 1);
        vec![("probe", Outcome::Counted(n, Invalidation::All))]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_op_parity_harness_matches_local_for_an_already_diverted_card_archive_json() {
    op_parity(Backend::Json, seed_graph, |ctx, s| {
        vec![
            (
                "archive_card",
                Outcome::unit(ctx.archive_card_impl(s.card_a)),
            ),
            (
                "restore_card",
                Outcome::card(ctx.restore_card_impl(s.card_a, None)),
            ),
        ]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_op_parity_harness_matches_local_for_an_already_diverted_card_archive_sqlite() {
    op_parity(Backend::Sqlite, seed_graph, |ctx, s| {
        vec![
            (
                "archive_card",
                Outcome::unit(ctx.archive_card_impl(s.card_a)),
            ),
            (
                "restore_card",
                Outcome::card(ctx.restore_card_impl(s.card_a, None)),
            ),
        ]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_no_children_over_http_returns_the_same_invalidation_as_local() {
    let dir = tempfile::tempdir().unwrap();
    let remote_path = dir.path().join("remote.store");
    let local_path = dir.path().join("local.store");

    let mut remote_seed = open_local(Backend::Json, &remote_path).await;
    let remote_seeded = seed_graph(&mut remote_seed);
    remote_seed.save().await.unwrap();
    drop(remote_seed);

    let mut local_seed = open_local(Backend::Json, &local_path).await;
    let local_seeded = seed_graph(&mut local_seed);
    local_seed.save().await.unwrap();

    let server = start_server(Backend::Json, &remote_path).await;
    let mut remote = ctx_over(&server).await;
    let remote_invalidation = remote
        .attach_children_impl(remote_seeded.card_a, vec![])
        .unwrap();
    drop(remote);
    server.shutdown().await;

    let local_invalidation = local_seed
        .attach_children_impl(local_seeded.card_a, vec![])
        .unwrap();

    assert_eq!(
        remote_invalidation, local_invalidation,
        "attaching an empty children list must invalidate the same thing over \
         HTTP as it does locally"
    );
}

async fn remote_graph_ops(kind: Backend) {
    op_parity(kind, seed_graph, |ctx, s: &Seeded| {
        vec![
            (
                "attach_children",
                Outcome::invalidated(ctx.attach_children_impl(s.card_a, vec![s.card_b])),
            ),
            (
                "block",
                Outcome::invalidated(ctx.block_impl(s.card_c, s.card_a, Severity::High)),
            ),
            (
                "relate_duplicates",
                Outcome::invalidated(ctx.relate_impl(s.card_b, s.card_c, RelatesKind::Duplicates)),
            ),
            (
                "relate_general",
                Outcome::invalidated(ctx.relate_impl(s.card_a, s.card_c, RelatesKind::General)),
            ),
            (
                "unblock",
                Outcome::invalidated(ctx.unblock_impl(s.card_c, s.card_a)),
            ),
            (
                "dissociate",
                Outcome::invalidated(ctx.dissociate_impl(s.card_a, s.card_c)),
            ),
            (
                "detach_children",
                Outcome::invalidated(ctx.detach_children_impl(s.card_a, vec![s.card_c])),
            ),
        ]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_graph_mutations_leave_graph_equal_to_local_json() {
    remote_graph_ops(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_graph_mutations_leave_graph_equal_to_local_sqlite() {
    remote_graph_ops(Backend::Sqlite).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_seed_graph_leaves_a_non_trivial_graph() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("seed.store");
    let mut ctx = open_local(Backend::Json, &path).await;
    let seeded = seed_graph(&mut ctx);

    assert_ne!(seeded.board_id, Uuid::nil(), "board_id");
    assert_ne!(seeded.todo, Uuid::nil(), "todo");
    assert_ne!(seeded.done, Uuid::nil(), "done");
    assert_ne!(seeded.card_a, Uuid::nil(), "card_a");
    assert_ne!(seeded.card_b, Uuid::nil(), "card_b");
    assert_ne!(seeded.card_c, Uuid::nil(), "card_c");
    assert_ne!(seeded.card_d, Uuid::nil(), "card_d");
    assert_ne!(seeded.sprint_id, Uuid::nil(), "sprint_id");
    assert_ne!(seeded.other_board_id, Uuid::nil(), "other_board_id");
    assert_ne!(seeded.backlog, Uuid::nil(), "backlog");
    assert_ne!(seeded.card_e, Uuid::nil(), "card_e");

    let ds = ctx.data_store();
    assert_eq!(ds.list_boards().unwrap().len(), 2, "boards");
    assert_eq!(ds.list_all_columns().unwrap().len(), 3, "columns");
    assert_eq!(ds.list_all_cards().unwrap().len(), 4, "live cards");
    assert_eq!(ds.list_all_sprints().unwrap().len(), 1, "sprints");
    assert_eq!(ds.list_archived_cards().unwrap().len(), 1, "archived_cards");
    assert!(
        ds.get_card(seeded.card_b).unwrap().is_some(),
        "archived card row must still exist under the reference-marker model"
    );

    let graph = ds.get_graph().unwrap();
    assert_eq!(graph.spawns_edges().len(), 1, "spawns edges");
    assert_eq!(graph.blocks_edges().len(), 1, "blocks edges");
    assert_eq!(graph.relates_edges().len(), 1, "relates edges");
}

async fn card_move_and_sprint_parity(kind: Backend) {
    op_parity(kind, seed_graph, |ctx, s| {
        vec![
            (
                "move_card_to_done",
                Outcome::card(ctx.move_card_impl(s.card_a, s.done, None)),
            ),
            (
                "move_card_to_todo",
                Outcome::card(ctx.move_card_impl(s.card_a, s.todo, Some(0))),
            ),
            (
                "unassign_card_from_sprint",
                Outcome::card(ctx.unassign_card_from_sprint_impl(s.card_c)),
            ),
            (
                "assign_card_to_sprint",
                Outcome::card(ctx.assign_card_to_sprint_impl(s.card_a, s.sprint_id)),
            ),
        ]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_move_and_sprint_binding_leave_graph_equal_to_local_json() {
    card_move_and_sprint_parity(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_move_and_sprint_binding_leave_graph_equal_to_local_sqlite() {
    card_move_and_sprint_parity(Backend::Sqlite).await;
}

async fn batch_ops_parity(kind: Backend) {
    op_parity(kind, seed_graph, |ctx, s| {
        vec![
            (
                "move_cards",
                Outcome::counted(ctx.move_cards_impl(vec![s.card_a], s.done)),
            ),
            (
                "assign_cards_to_sprint",
                Outcome::counted(ctx.assign_cards_to_sprint_impl(vec![s.card_a], s.sprint_id)),
            ),
            (
                "update_cards",
                Outcome::counted(ctx.update_cards_impl(vec![(
                    s.card_a,
                    CardUpdate {
                        title: Some("A2".into()),
                        ..Default::default()
                    },
                )])),
            ),
            (
                "archive_cards",
                Outcome::counted(ctx.archive_cards_impl(vec![s.card_a])),
            ),
        ]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_batch_ops_leave_graph_equal_to_local_json() {
    batch_ops_parity(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_batch_ops_leave_graph_equal_to_local_sqlite() {
    batch_ops_parity(Backend::Sqlite).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_card_over_http_issues_exactly_one_request() {
    let mut seeded_slot: Option<Seeded> = None;
    let (server, log) = TestServer::start_recording(|ctx| {
        seeded_slot = Some(seed_graph(ctx));
    })
    .await;
    let seeded = seeded_slot.unwrap();

    let mut ctx = ctx_over(&server).await;
    let before = log.lock().unwrap().len();

    let _ = ctx
        .move_card_impl(seeded.card_a, seeded.done, None)
        .unwrap();

    let issued = log.lock().unwrap()[before..].to_vec();
    assert_eq!(
        issued.len(),
        1,
        "move_card_impl must issue exactly one HTTP request beyond whatever \
         KanbanContext::open already issued, got {issued:?}"
    );
    assert_eq!(issued[0].0.to_string(), "POST");
    assert_eq!(issued[0].1, format!("/v1/cards/{}/move", seeded.card_a));

    drop(ctx);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_card_to_sprint_over_http_issues_exactly_one_request() {
    let mut seeded_slot: Option<Seeded> = None;
    let (server, log) = TestServer::start_recording(|ctx| {
        seeded_slot = Some(seed_graph(ctx));
    })
    .await;
    let seeded = seeded_slot.unwrap();

    let mut ctx = ctx_over(&server).await;
    let before = log.lock().unwrap().len();

    let _ = ctx
        .assign_card_to_sprint_impl(seeded.card_a, seeded.sprint_id)
        .unwrap();

    let issued = log.lock().unwrap()[before..].to_vec();
    assert_eq!(
        issued.len(),
        1,
        "assign_card_to_sprint_impl must issue exactly one HTTP request beyond \
         whatever KanbanContext::open already issued, got {issued:?}"
    );
    assert_eq!(issued[0].0.to_string(), "POST");
    assert_eq!(
        issued[0].1,
        format!("/v1/cards/{}/assign-sprint", seeded.card_a)
    );

    drop(ctx);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unassign_card_from_sprint_over_http_issues_exactly_one_request() {
    let mut seeded_slot: Option<Seeded> = None;
    let (server, log) = TestServer::start_recording(|ctx| {
        seeded_slot = Some(seed_graph(ctx));
    })
    .await;
    let seeded = seeded_slot.unwrap();

    let mut ctx = ctx_over(&server).await;
    let before = log.lock().unwrap().len();

    let _ = ctx.unassign_card_from_sprint_impl(seeded.card_c).unwrap();

    let issued = log.lock().unwrap()[before..].to_vec();
    assert_eq!(
        issued.len(),
        1,
        "unassign_card_from_sprint_impl must issue exactly one HTTP request beyond \
         whatever KanbanContext::open already issued, got {issued:?}"
    );
    assert_eq!(issued[0].0.to_string(), "POST");
    assert_eq!(
        issued[0].1,
        format!("/v1/cards/{}/unassign-sprint", seeded.card_c)
    );

    drop(ctx);
    server.shutdown().await;
}

const S1: Uuid = Uuid::from_u128(0x5171);
const S2: Uuid = Uuid::from_u128(0x5172);

fn sprint_crud_ops(ctx: &mut KanbanContext, s: &Seeded) -> Outcomes {
    vec![
        (
            "create_sprint_named",
            Outcome::sprint(ctx.create_sprint_from_spec(
                s.board_id,
                Some(S1),
                Some("Named".into()),
                None,
                false,
            )),
        ),
        (
            "create_sprint_alt_prefix",
            Outcome::sprint(ctx.create_sprint_from_spec(
                s.board_id,
                Some(S2),
                None,
                Some("ALT".into()),
                false,
            )),
        ),
        (
            "update_sprint",
            Outcome::sprint(ctx.update_sprint_impl(
                S1,
                SprintUpdate {
                    name: Some("Renamed".into()),
                    start_date: FieldUpdate::Set(fixed_due()),
                    ..Default::default()
                },
            )),
        ),
        (
            "delete_sprint",
            Outcome::invalidated(ctx.delete_sprint_impl(s.sprint_id)),
        ),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_sprint_crud_leaves_graph_equal_to_local_json() {
    op_parity(Backend::Json, seed_graph, sprint_crud_ops).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_sprint_crud_leaves_graph_equal_to_local_sqlite() {
    op_parity(Backend::Sqlite, seed_graph, sprint_crud_ops).await;
}

struct LifecycleSeeded {
    base: Seeded,
    second_sprint: Uuid,
}

fn seed_lifecycle_graph(ctx: &mut KanbanContext) -> LifecycleSeeded {
    let base = seed_graph(ctx);
    ctx.assign_card_to_sprint(base.card_a, base.sprint_id)
        .unwrap();
    let second_sprint = ctx.create_sprint(base.board_id, None, None).unwrap().id;
    LifecycleSeeded {
        base,
        second_sprint,
    }
}

fn sprint_lifecycle_ops(ctx: &mut KanbanContext, s: &LifecycleSeeded) -> Outcomes {
    let activate = Outcome::sprint(ctx.activate_sprint_impl(s.base.sprint_id, Some(7)));
    let complete = Outcome::sprint(ctx.complete_sprint_impl(s.base.sprint_id));
    let carry_over =
        Outcome::counted(ctx.carry_over_sprint_cards_impl(s.base.sprint_id, s.second_sprint));
    assert!(
        matches!(carry_over, Outcome::Counted(2, _)),
        "card_a and card_c are uncompleted in the first sprint: {carry_over:?}"
    );
    let cancel = Outcome::sprint(ctx.cancel_sprint_impl(s.second_sprint));
    vec![
        ("activate_sprint", activate),
        ("complete_sprint", complete),
        ("carry_over_sprint_cards", carry_over),
        ("cancel_sprint", cancel),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_sprint_lifecycle_leaves_graph_equal_to_local_json() {
    op_parity(Backend::Json, seed_lifecycle_graph, sprint_lifecycle_ops).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_sprint_lifecycle_leaves_graph_equal_to_local_sqlite() {
    op_parity(Backend::Sqlite, seed_lifecycle_graph, sprint_lifecycle_ops).await;
}

fn issued_since(
    log: &kanban_server::test_helpers::RequestLog,
    from: usize,
) -> Vec<(String, String)> {
    log.lock().unwrap()[from..]
        .iter()
        .map(|(method, path)| (method.to_string(), path.clone()))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_lifecycle_over_http_issues_one_flat_write_and_at_most_one_board_read() {
    let mut seeded_slot: Option<LifecycleSeeded> = None;
    let (server, log) = TestServer::start_recording(|ctx| {
        seeded_slot = Some(seed_lifecycle_graph(ctx));
    })
    .await;
    let s = seeded_slot.unwrap();
    let (first, second, board_id) = (s.base.sprint_id, s.second_sprint, s.base.board_id);
    let post = |path: String| ("POST".to_string(), path);
    let board_read = ("GET".to_string(), format!("/v1/boards/{board_id}"));
    let mut ctx = ctx_over(&server).await;

    let mark = log.lock().unwrap().len();
    let _ = ctx.activate_sprint_impl(first, Some(7)).unwrap();
    assert_eq!(
        issued_since(&log, mark),
        vec![
            post(format!("/v1/sprints/{first}/activate")),
            board_read.clone()
        ]
    );

    let mark = log.lock().unwrap().len();
    let _ = ctx.complete_sprint_impl(first).unwrap();
    assert_eq!(
        issued_since(&log, mark),
        vec![
            post(format!("/v1/sprints/{first}/complete")),
            board_read.clone()
        ]
    );

    let mark = log.lock().unwrap().len();
    let _ = ctx.carry_over_sprint_cards_impl(first, second).unwrap();
    assert_eq!(
        issued_since(&log, mark),
        vec![post(format!("/v1/sprints/{first}/carry-over"))]
    );

    let mark = log.lock().unwrap().len();
    let _ = ctx.cancel_sprint_impl(second).unwrap();
    assert_eq!(
        issued_since(&log, mark),
        vec![post(format!("/v1/sprints/{second}/cancel")), board_read]
    );

    drop(ctx);
    server.shutdown().await;
}
