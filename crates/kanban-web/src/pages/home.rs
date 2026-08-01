use crate::context::SharedCtx;
use topcoat::{
    context::{app_context, Cx},
    router::page,
    view::view,
    Result,
};

#[page("/")]
async fn home(cx: &Cx) -> Result {
    let ctx: &SharedCtx = app_context(cx);
    let ctx = ctx.lock().await;
    let boards = ctx.boards()?;

    view! {
        <!DOCTYPE html>
        <html>
            <body>
                <h1>"Boards"</h1>
                for board in &boards {
                    <section>
                        <h2>(board.name.clone())</h2>
                        for column in ctx.data_store().list_columns_by_board(board.id)? {
                            <div>
                                <h3>(column.name.clone())</h3>
                                <ul>
                                    for card in ctx.data_store().list_cards_by_column(column.id)? {
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
