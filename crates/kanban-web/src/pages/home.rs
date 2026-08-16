use crate::context::SharedCtx;
use kanban_domain::card_lifecycle::sorted_board_columns;
use kanban_domain::CardQueryBuilder;
use kanban_service::KanbanContext;
use kanban_view::model::Model;
use topcoat::{
    context::{app_context, Cx},
    router::page,
    view::view,
    Result,
};

struct CardView {
    title: String,
}

struct ColumnView {
    name: String,
    cards: Vec<CardView>,
}

struct BoardView {
    name: String,
    columns: Vec<ColumnView>,
}

/// Synchronous DataStore reads for the whole page, run off the async
/// executor via `spawn_blocking` in `home` below. `HttpBackend`'s DataStore
/// methods bridge onto their own dedicated runtime and panic ("Cannot start
/// a runtime from within a runtime") if called directly from an
/// already-async caller -- which a topcoat page handler is.
///
/// Reads boards/columns/cards individually (not `ctx.snapshot()`, which
/// `HttpBackend` doesn't implement — only the piecemeal per-board/column
/// reads used here) and feeds them into a `kanban_view::model::Model` so
/// column ordering and card filtering/sorting come from the same shared
/// logic `kanban-tui` uses, rather than being re-derived here. Archived
/// boards/cards and the dependency graph are left empty, matching
/// `HttpBackend`'s other documented read-path gaps.
fn load_boards(ctx: &KanbanContext) -> kanban_service::KanbanResult<Vec<BoardView>> {
    let boards = ctx.boards()?;
    let mut columns = Vec::new();
    let mut cards = Vec::new();
    for board in &boards {
        let board_columns = ctx.data_store().list_columns_by_board(board.id)?;
        for column in &board_columns {
            cards.extend(ctx.data_store().list_cards_by_column(column.id)?);
        }
        columns.extend(board_columns);
    }

    let mut model = Model::default();
    model.load_from_snapshot(kanban_domain::Snapshot {
        boards: boards.clone(),
        columns,
        cards,
        archived_cards: Vec::new(),
        sprints: Vec::new(),
        archived_boards: Vec::new(),
        graph: Default::default(),
    });

    Ok(boards
        .iter()
        .map(|board| {
            let columns = sorted_board_columns(board.id, model.columns())
                .into_iter()
                .map(|column| {
                    let card_ids = CardQueryBuilder::new(
                        model.all_cards(),
                        model.columns(),
                        model.sprints(),
                        board,
                    )
                    .in_column(column.id)
                    .execute();
                    let cards = card_ids
                        .into_iter()
                        .filter_map(|id| model.card_by_id(id))
                        .map(|card| CardView {
                            title: card.title.clone(),
                        })
                        .collect();
                    ColumnView {
                        name: column.name.clone(),
                        cards,
                    }
                })
                .collect();
            BoardView {
                name: board.name.clone(),
                columns,
            }
        })
        .collect())
}

#[page("/")]
async fn home(cx: &Cx) -> Result {
    let ctx: SharedCtx = app_context::<SharedCtx>(cx).clone();
    let boards = tokio::task::spawn_blocking(move || {
        let ctx = ctx.blocking_lock();
        load_boards(&ctx)
    })
    .await
    .expect("home page's blocking DataStore read panicked")?;

    view! {
        <!DOCTYPE html>
        <html>
            <body>
                <h1>"Boards"</h1>
                for board in &boards {
                    <section>
                        <h2>(board.name.clone())</h2>
                        for column in &board.columns {
                            <div>
                                <h3>(column.name.clone())</h3>
                                <ul>
                                    for card in &column.cards {
                                        <li>(card.title.clone())</li>
                                    }
                                </ul>
                            </div>
                        }
                    </section>
                }
            </body>
        </html>
    }
}
