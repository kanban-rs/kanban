use crate::app::App;
use crate::components::*;
use ratatui::Frame;

pub(crate) fn render_create_sprint_popup(app: &App, frame: &mut Frame) {
    render_input_popup(
        frame,
        "Create New Sprint",
        "Sprint Name (optional):",
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    );
}

pub(crate) fn render_set_sprint_prefix_popup(app: &App, frame: &mut Frame) {
    render_input_popup(
        frame,
        "Set Sprint Prefix",
        "Sprint Prefix:",
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    );
}

pub(crate) fn render_set_sprint_card_prefix_popup(app: &App, frame: &mut Frame) {
    render_input_popup(
        frame,
        "Set Card Prefix Override",
        "Card Prefix:",
        app.input.as_str(),
        app.input.cursor_byte_offset(),
    );
}

pub(crate) fn render_carry_over_sprint_popup(app: &App, frame: &mut Frame) {
    use crate::components::selection_dialog::CarryOverSprintDialog;
    use crate::components::SelectionDialog;
    let card_count = app
        .dialog_input
        .carry_over_source_sprint_id
        .map(|id| {
            use kanban_domain::query::sprint::get_sprint_uncompleted_cards;
            get_sprint_uncompleted_cards(id, app.model.live_cards()).len()
        })
        .unwrap_or(0);
    CarryOverSprintDialog { card_count }.render(app, frame);
}
