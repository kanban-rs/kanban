use super::App;
use kanban_service::git::CommitRef;

/// Cached result of the one-shot git-log lookup for the card currently open
/// in card detail. Populated on detail open / active-card change; rendered
/// from cache every frame (never re-shelled per frame).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CommitsPanel {
    #[default]
    NotLoaded,
    Loaded(Vec<CommitRef>),
    Unavailable,
}

impl App {
    #[doc(hidden)]
    pub fn set_git_provider(
        &mut self,
        provider: Option<Box<dyn kanban_service::git::GitProvider>>,
    ) {
        self.git_provider = provider;
    }

    /// Recompute `commits_panel` for the card currently open in card detail.
    /// Invoked only on detail open / active-card change, never per render frame.
    /// `None` provider or a provider error → Unavailable.
    #[doc(hidden)]
    pub fn refresh_card_commits(&mut self) {
        self.commits_panel = self.compute_card_commits();
    }

    fn compute_card_commits(&self) -> CommitsPanel {
        let Some(provider) = self.git_provider.as_ref() else {
            return CommitsPanel::Unavailable;
        };
        let Some(card) = self.get_card_for_detail_view() else {
            return CommitsPanel::NotLoaded;
        };
        let Some(board) = self.active_board() else {
            return CommitsPanel::NotLoaded;
        };
        let tag = card.identifier(
            board,
            self.model.sprints(),
            self.app_config.effective_default_card_prefix(),
        );
        match provider.commits_for_tag(&tag) {
            Ok(commits) => CommitsPanel::Loaded(commits),
            Err(_) => CommitsPanel::Unavailable,
        }
    }
}
