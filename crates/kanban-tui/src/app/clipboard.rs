use super::App;
use crate::clipboard;
use kanban_domain::{Board, Card, Sprint};

impl App {
    /// Generic handler for copying card outputs to clipboard
    fn copy_card_output<F>(&mut self, output_type: &str, get_output: F)
    where
        F: Fn(&Card, &Board, &[Sprint], &str) -> String,
    {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(board) = self.active_board() {
                if let Some(card) = self.model.card_by_id(active_id) {
                    let sprints = self.model.sprints();
                    let output = get_output(
                        card,
                        board,
                        sprints,
                        self.app_config.effective_default_card_prefix(),
                    );
                    if let Err(e) = clipboard::copy_to_clipboard(&output) {
                        self.set_error(format!("Failed to copy: {}", e));
                    } else {
                        self.set_success(format!("Copied {}", output_type));
                    }
                }
            }
        }
    }

    pub fn copy_branch_name(&mut self) {
        self.copy_card_output("branch name", |card, board, sprints, prefix| {
            card.branch_name(board, sprints, prefix)
        });
    }

    pub fn copy_git_checkout_command(&mut self) {
        self.copy_card_output("command", |card, board, sprints, prefix| {
            card.git_checkout_command(board, sprints, prefix)
        });
    }
}
