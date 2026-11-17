//! State for the "Manage Labels" card dialog.
//!
//! The dialog renders two sections plus a create row in a single flat cursor
//! list: the card's applied labels (upper), the global label registry (lower),
//! and a "new label" row at the bottom. [`LabelManagerState::resolve`] maps a
//! key to a semantic [`LabelAction`] based on which row the cursor is on, so
//! the apply/remove/delete/create decision matrix is pure and unit-testable.
use kanban_core::SelectionState;
use kanban_domain::Label;

/// Which row the flat cursor is pointing at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelRow {
    /// Upper section: a label currently applied to the card. Index into
    /// [`LabelManagerState::applied`].
    Applied(usize),
    /// Lower section: a label from the global registry (may or may not be
    /// applied to the card). Index into [`LabelManagerState::registry`].
    Registry(usize),
    /// Bottom row: create a new label.
    Create,
}

/// The semantic result of a key press in the dialog. Drives the handler that
/// turns it into a service call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelAction {
    /// No-op key (anything unhandled).
    None,
    /// Remove this label from the card (registry entry left intact).
    RemoveFromCard(String),
    /// Add this label to the card (auto-registers it if missing).
    ApplyToCard(String),
    /// Delete this label from the registry (and, if applied, from the card).
    DeleteLabel(String),
    /// Open the external editor to rename / edit this label.
    EditLabel(String),
    /// Open the external editor to create a new label.
    CreateLabel,
    /// Move the cursor down.
    NavigateDown,
    /// Move the cursor up.
    NavigateUp,
    /// Close the dialog.
    Close,
}

/// Flat-list cursor state for [`super::DialogMode::ManageLabels`].
pub struct LabelManagerState {
    /// Label names currently applied to the managed card (`Card.tags`),
    /// in the card's own order.
    pub applied: Vec<String>,
    /// Snapshot of the global label registry, taken when the dialog opened.
    pub registry: Vec<Label>,
    /// Shared cursor over the flat list (applied, then registry, then create).
    pub selection: SelectionState,
}

impl Default for LabelManagerState {
    fn default() -> Self {
        Self {
            applied: Vec::new(),
            registry: Vec::new(),
            selection: SelectionState::new(),
        }
    }
}

impl LabelManagerState {
    /// Total number of selectable rows (applied + registry + the create row).
    pub fn total_rows(&self) -> usize {
        self.applied.len() + self.registry.len() + 1
    }

    /// The row at a flat index, or `None` if out of range.
    pub fn row_at(&self, idx: usize) -> Option<LabelRow> {
        let applied_len = self.applied.len();
        if idx < applied_len {
            Some(LabelRow::Applied(idx))
        } else if idx < applied_len + self.registry.len() {
            Some(LabelRow::Registry(idx - applied_len))
        } else if idx == applied_len + self.registry.len() {
            Some(LabelRow::Create)
        } else {
            None
        }
    }

    /// The row the cursor is on right now.
    pub fn current_row(&self) -> Option<LabelRow> {
        self.selection.get().and_then(|i| self.row_at(i))
    }

    /// Move the cursor down, clamped to the last row.
    pub fn navigate_down(&mut self) {
        self.selection.next(self.total_rows());
    }

    /// Move the cursor up, clamped to the first row.
    pub fn navigate_up(&mut self) {
        self.selection.prev();
    }

    /// Re-read the applied set and registry after a mutation, clamping the
    /// cursor into the (possibly smaller) new row count.
    pub fn refresh(&mut self, applied: Vec<String>, registry: Vec<Label>) {
        let selected = self.selection.get();
        self.applied = applied;
        self.registry = registry;
        if let Some(i) = selected {
            self.selection.set(Some(i.min(self.total_rows() - 1)));
        }
    }

    /// Map a key press to the action the cursor row demands. Pure read-only,
    /// so it can be exercised in isolation.
    pub fn resolve(&self, key: crossterm::event::KeyCode) -> LabelAction {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Esc => LabelAction::Close,
            KeyCode::Char('j') | KeyCode::Down => LabelAction::NavigateDown,
            KeyCode::Char('k') | KeyCode::Up => LabelAction::NavigateUp,
            // Create is reachable from any row.
            KeyCode::Char('n') | KeyCode::Char('N') => LabelAction::CreateLabel,
            // `e` edits the label under the cursor (or creates when on the
            // create row).
            KeyCode::Char('e') => match self.current_row() {
                Some(LabelRow::Applied(i)) => LabelAction::EditLabel(self.applied[i].clone()),
                Some(LabelRow::Registry(i)) => {
                    LabelAction::EditLabel(self.registry[i].name.clone())
                }
                Some(LabelRow::Create) => LabelAction::CreateLabel,
                None => LabelAction::None,
            },
            // `x` removes: from the card (upper) or from the registry (lower).
            KeyCode::Char('x') => match self.current_row() {
                Some(LabelRow::Applied(i)) => LabelAction::RemoveFromCard(self.applied[i].clone()),
                Some(LabelRow::Registry(i)) => {
                    LabelAction::DeleteLabel(self.registry[i].name.clone())
                }
                _ => LabelAction::None,
            },
            // Enter/space: set contextually. On an applied label it removes it
            // from the card; on a registry label it applies it to the card; on
            // the create row it opens the create editor.
            KeyCode::Enter | KeyCode::Char(' ') => match self.current_row() {
                Some(LabelRow::Applied(i)) => LabelAction::RemoveFromCard(self.applied[i].clone()),
                Some(LabelRow::Registry(i)) => {
                    LabelAction::ApplyToCard(self.registry[i].name.clone())
                }
                Some(LabelRow::Create) => LabelAction::CreateLabel,
                None => LabelAction::None,
            },
            _ => LabelAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(name: &str) -> Label {
        Label::named(name)
    }

    fn state(applied: Vec<&str>, registry: Vec<&str>) -> LabelManagerState {
        LabelManagerState {
            applied: applied.into_iter().map(|s| s.to_string()).collect(),
            registry: registry.into_iter().map(|s| label(s)).collect(),
            selection: SelectionState::new(),
        }
    }

    #[test]
    fn resolve_applied_row_enter_removes_from_card() {
        let mut s = state(vec!["bug", "p1"], vec!["bug"]);
        s.selection.set(Some(0));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::RemoveFromCard("bug".into())
        );
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('x')),
            LabelAction::RemoveFromCard("bug".into())
        );
    }

    #[test]
    fn resolve_registry_row_enter_applies_to_card() {
        let mut s = state(vec!["bug"], vec!["bug", "design"]);
        // "design" is registry index 1 -> flat index applied.len()+1 = 2
        s.selection.set(Some(2));
        assert_eq!(s.row_at(2), Some(LabelRow::Registry(1)));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::ApplyToCard("design".into())
        );
    }

    #[test]
    fn resolve_registry_row_x_deletes_label() {
        let mut s = state(vec![], vec!["a", "b"]);
        s.selection.set(Some(1));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('x')),
            LabelAction::DeleteLabel("b".into())
        );
    }

    #[test]
    fn resolve_create_row_opens_create() {
        let mut s = state(vec![], vec!["a"]);
        s.selection.set(Some(1)); // create row is last
        assert_eq!(s.total_rows(), 2);
        assert_eq!(s.row_at(1), Some(LabelRow::Create));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::CreateLabel
        );
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('n')),
            LabelAction::CreateLabel
        );
    }

    #[test]
    fn resolve_e_edits_label_under_cursor() {
        let mut s = state(vec!["bug"], vec!["bug", "design"]);
        s.selection.set(Some(0));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::EditLabel("bug".into())
        );
        s.selection.set(Some(2));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::EditLabel("design".into())
        );
    }

    #[test]
    fn resolve_navigation_clamps_to_bounds() {
        let mut s = state(vec![], vec!["a", "b"]);
        assert_eq!(s.total_rows(), 3);
        s.selection.set(Some(0));
        s.navigate_up();
        assert_eq!(s.selection.get(), Some(0), "up at top stays at top");
        s.selection.set(Some(2));
        s.navigate_down();
        assert_eq!(s.selection.get(), Some(2), "down at bottom stays at bottom");
    }

    #[test]
    fn resolve_unhandled_key_is_noop() {
        let s = state(vec!["a"], vec!["a"]);
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('q')),
            LabelAction::None
        );
    }

    #[test]
    fn refresh_clamps_selection_into_new_range() {
        let mut s = state(vec!["a", "b", "c"], vec!["a"]);
        s.selection.set(Some(2));
        s.refresh(vec!["a".to_string()], vec![Label::named("a")]);
        assert!(s
            .selection
            .get()
            .map(|i| i < s.total_rows())
            .unwrap_or(true));
    }
}
