use crate::context::SharedCtx;
use kanban_service::KanbanContext;
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
fn load_boards(ctx: &KanbanContext) -> kanban_service::KanbanResult<Vec<BoardView>> {
    ctx.boards()?
        .into_iter()
        .map(|board| {
            let columns = ctx
                .data_store()
                .list_columns_by_board(board.id)?
                .into_iter()
                .map(|column| {
                    let cards = ctx
                        .data_store()
                        .list_cards_by_column(column.id)?
                        .into_iter()
                        .map(|card| CardView { title: card.title })
                        .collect();
                    Ok(ColumnView {
                        name: column.name,
                        cards,
                    })
                })
                .collect::<kanban_service::KanbanResult<Vec<_>>>()?;
            Ok(BoardView {
                name: board.name,
                columns,
            })
        })
        .collect()
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
