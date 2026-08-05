#![allow(dead_code)]

use crate::app::App;
use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations, Snapshot};
use std::sync::Mutex;

/// Every test in this binary that mutates a process-global environment
/// variable (`HOME`, `KANBAN_CONFIG`, ...) must serialize on this lock, not
/// a var-specific one — `setenv`/`getenv` are unsynchronized against each
/// other in the same process regardless of which variable each call names.
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn load_with_card_order(app: &mut App, order: &[uuid::Uuid]) {
    let all = app.ctx.data_store().list_all_cards().unwrap();
    let ordered: Vec<_> = order
        .iter()
        .map(|id| {
            all.iter()
                .find(|c| c.id == *id)
                .cloned()
                .expect("card id present in store")
        })
        .collect();
    let snap = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: ordered,
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snap);
}

pub struct ReloadResortFixture {
    pub board_id: uuid::Uuid,
    pub column_id: uuid::Uuid,
    pub a_id: uuid::Uuid,
    pub p_id: uuid::Uuid,
    pub d_id: uuid::Uuid,
    pub b_id: uuid::Uuid,
    pub c_id: uuid::Uuid,
}

/// Simulates the KAN-534 scenario: an external write triggers a TUI
/// reload that reorders `model.all_cards()`, leaving `ActiveCard.index`
/// pointing at a different card than `ActiveCard.id`.
///
/// Seeds five cards in the same column with edges P -> A -> D, sets the
/// active card to A at index 1, then re-loads the model with cards in a
/// different order so index 1 now resolves to P (not A). Any production
/// site that still resolves the active card by index will silently
/// operate on P; sites that resolve by id will still operate on A.
pub fn setup_reload_resort_fixture(app: &mut App) -> ReloadResortFixture {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let p = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "P".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let a = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let c = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "C".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let d = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "D".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.attach_child(p.id, a.id).unwrap();
    app.ctx.attach_child(a.id, d.id).unwrap();

    load_with_card_order(app, &[p.id, a.id, b.id, c.id, d.id]);
    app.selection.active_card_id = Some(a.id);
    app.selection.active_board_id = app.model.boards().first().map(|b| b.id);

    load_with_card_order(app, &[a.id, p.id, b.id, c.id, d.id]);

    ReloadResortFixture {
        board_id: board.id,
        column_id: column.id,
        a_id: a.id,
        p_id: p.id,
        d_id: d.id,
        b_id: b.id,
        c_id: c.id,
    }
}
