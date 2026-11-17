//! State for the "Manage Labels" card dialog.
//!
//! The dialog is a single flat list (the same pattern as the sprint picker and
//! the relationship lists): section headers are non-selectable rows
//! interspersed between the selectable items, so `j`/`k` navigation skims over
//! them and keeps the cursor on a real action row. [`LabelManagerState::entries`]
//! is the source of truth for that flat ordering, and
//! [`LabelManagerState::resolve`] maps a key to a semantic [`LabelAction`]
//! based on whichever row the cursor is on, so the apply/remove/delete/create
//! decision matrix stays pure and unit-testable.
use kanban_core::SelectionState;
use kanban_domain::Label;
use uuid::Uuid;

/// Which row the flat cursor is pointing at.
///
/// `Header` and `Note` are non-selectable: the cursor never lands on them and
/// they exist purely for layout (section titles and muted empty-state hints).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelRow {
    /// Section header label (e.g. "Applied to this card").
    Header(&'static str),
    /// Muted note shown when a section is empty (e.g. "(none)").
    Note(&'static str),
    /// A label currently applied to the managed card. Index into
    /// [`LabelManagerState::applied`].
    Applied(usize),
    /// A label from the global registry (may or may not be applied). Index
    /// into [`LabelManagerState::registry`].
    Registry(usize),
    /// The "new label" row at the bottom of the list.
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
    /// Move the cursor down (resolve already performed the move).
    NavigateDown,
    /// Move the cursor up (resolve already performed the move).
    NavigateUp,
    /// Close the dialog.
    Close,
}

/// Flat-list cursor state for [`super::DialogMode::ManageLabels`].
pub struct LabelManagerState {
    /// The cards this dialog manages. Usually one (opened from a card's detail
    /// view) but may be several when opened from the card list with multi-select
    /// active, in which case every change applies to all of them.
    pub card_ids: Vec<Uuid>,
    /// Label names currently applied to the managed card(s) (`Card.tags`), in
    /// the first card's own order. For multiple cards this is the intersection
    /// of their tags — the labels common to all of them.
    pub applied: Vec<String>,
    /// Snapshot of the global label registry, taken when the dialog opened.
    pub registry: Vec<Label>,
    /// Shared cursor over the flat [`Self::entries`] list.
    pub selection: SelectionState,
}

impl Default for LabelManagerState {
    fn default() -> Self {
        Self {
            card_ids: Vec::new(),
            applied: Vec::new(),
            registry: Vec::new(),
            selection: SelectionState::new(),
        }
    }
}

fn is_selectable(row: LabelRow) -> bool {
    matches!(
        row,
        LabelRow::Applied(_) | LabelRow::Registry(_) | LabelRow::Create
    )
}

/// The labels common to every supplied card, kept in the first card's own tag
/// order. The "Applied" section of the dialog shows exactly this: one card
/// means its own tags; several cards means the labels all of them share.
pub fn intersect_tags(per_card: &[Vec<String>]) -> Vec<String> {
    let first = match per_card.first() {
        Some(v) => v,
        None => return Vec::new(),
    };
    first
        .iter()
        .filter(|tag| per_card[1..].iter().all(|tags| tags.contains(tag)))
        .cloned()
        .collect()
}

impl LabelManagerState {
    /// The flat row list in layout order: an "Applied" header, then the
    /// applied rows (or a "(none)" note), a "Labels" header, then the registry
    /// rows (or a "(none defined yet)" note), and finally a "Create" header and
    /// the actionable create row.
    pub fn entries(&self) -> Vec<LabelRow> {
        let mut e = Vec::new();
        e.push(LabelRow::Header("Applied to this card"));
        if self.applied.is_empty() {
            e.push(LabelRow::Note("(none)"));
        } else {
            for i in 0..self.applied.len() {
                e.push(LabelRow::Applied(i));
            }
        }
        e.push(LabelRow::Header("Labels"));
        if self.registry.is_empty() {
            e.push(LabelRow::Note("(none defined yet)"));
        } else {
            for i in 0..self.registry.len() {
                e.push(LabelRow::Registry(i));
            }
        }
        e.push(LabelRow::Header("Create"));
        e.push(LabelRow::Create);
        e
    }

    /// Total number of flat rows (selectable items plus their headers/notes).
    pub fn total_rows(&self) -> usize {
        self.entries().len()
    }

    /// The row at a flat index, or `None` if out of range.
    pub fn row_at(&self, idx: usize) -> Option<LabelRow> {
        self.entries().get(idx).copied()
    }

    /// The row the cursor is on right now.
    pub fn current_row(&self) -> Option<LabelRow> {
        self.selection.get().and_then(|i| self.row_at(i))
    }

    /// The first selectable flat index.
    pub fn first_selectable(&self) -> Option<usize> {
        let entries = self.entries();
        kanban_view::list_nav::first_selectable_index(entries.len(), |i| is_selectable(entries[i]))
    }

    /// Move the cursor to the next selectable row, skimming over headers and
    /// notes, clamped at the last row.
    pub fn navigate_down(&mut self) {
        let entries = self.entries();
        if let Some(next) =
            kanban_view::list_nav::next_selectable_index(self.selection.get(), entries.len(), |i| {
                is_selectable(entries[i])
            })
        {
            self.selection.set(Some(next));
        }
    }

    /// Move the cursor to the previous selectable row, skimming over headers
    /// and notes, clamped at the first row.
    pub fn navigate_up(&mut self) {
        if let Some(prev) = kanban_view::list_nav::prev_selectable_index(
            self.selection.get(),
            self.entries().len(),
            |i| is_selectable(self.entries()[i]),
        ) {
            self.selection.set(Some(prev));
        }
    }

    /// Re-read the applied set and registry after a mutation, keeping the
    /// cursor on a still-valid selectable row and otherwise defaulting to the
    /// first selectable row.
    pub fn refresh(&mut self, applied: Vec<String>, registry: Vec<Label>) {
        self.applied = applied;
        self.registry = registry;
        let keep = self
            .selection
            .get()
            .filter(|i| self.entries().get(*i).copied().is_some_and(is_selectable));
        self.selection.set(keep.or_else(|| self.first_selectable()));
    }

    /// Map a key press to the action the cursor row demands. `j`/`k` move the
    /// cursor in-place, so the returned [`LabelAction::NavigateDown`]/`NavigateUp`
    /// arms are informational no-ops in the handler.
    pub fn resolve(&mut self, key: crossterm::event::KeyCode) -> LabelAction {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Esc => LabelAction::Close,
            KeyCode::Char('j') | KeyCode::Down => {
                self.navigate_down();
                LabelAction::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.navigate_up();
                LabelAction::NavigateUp
            }
            // `e` edits a registry label, from the Labels section. In the
            // Applied section editing belongs in the Labels section, and
            // creating is reserved for Enter/space on the create row, so `e`
            // is a no-op on every row except registry labels.
            KeyCode::Char('e') => match self.current_row() {
                Some(LabelRow::Registry(i)) => {
                    LabelAction::EditLabel(self.registry[i].name.clone())
                }
                Some(LabelRow::Applied(_))
                | Some(LabelRow::Create)
                | Some(LabelRow::Header(_))
                | Some(LabelRow::Note(_))
                | None => LabelAction::None,
            },
            // `x` deletes a registry label (from the Labels section). Removing
            // an applied label from the card is Enter's job, so `x` is a no-op
            // on applied rows and rows without a registry label.
            KeyCode::Char('x') => match self.current_row() {
                Some(LabelRow::Registry(i)) => {
                    LabelAction::DeleteLabel(self.registry[i].name.clone())
                }
                Some(LabelRow::Applied(_)) | Some(LabelRow::Create) | None => LabelAction::None,
                Some(LabelRow::Header(_)) | Some(LabelRow::Note(_)) => LabelAction::None,
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
                Some(LabelRow::Header(_)) | Some(LabelRow::Note(_)) | None => LabelAction::None,
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
            card_ids: Vec::new(),
            applied: applied.into_iter().map(|s| s.to_string()).collect(),
            registry: registry.into_iter().map(|s| label(s)).collect(),
            selection: SelectionState::new(),
        }
    }

    #[test]
    fn resolve_applied_row_enter_removes_only() {
        let mut s = state(vec!["bug", "p1"], vec!["bug"]);
        s.selection.set(Some(1)); // flat 1 = the only-applied "bug" row
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::RemoveFromCard("bug".into())
        );
        // In the Applied section only Enter acts; `x` and `e` are reserved
        // for the Labels section, so they are no-ops on applied rows.
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('x')),
            LabelAction::None
        );
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::None
        );
    }

    #[test]
    fn resolve_registry_row_enter_applies_to_card() {
        let mut s = state(vec!["bug"], vec!["bug", "design"]);
        // entries: 0 hdr, 1 Applied0, 2 hdr, 3 Registry0, 4 Registry1, 5 hdr, 6 Create
        s.selection.set(Some(4));
        assert_eq!(s.row_at(4), Some(LabelRow::Registry(1)));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::ApplyToCard("design".into())
        );
    }

    #[test]
    fn resolve_registry_row_x_deletes_label() {
        let mut s = state(vec![], vec!["a", "b"]);
        // entries: 0 hdr, 1 note, 2 hdr, 3 Registry0, 4 Registry1, 5 hdr, 6 Create
        s.selection.set(Some(4));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('x')),
            LabelAction::DeleteLabel("b".into())
        );
    }

    #[test]
    fn resolve_create_row_opens_create() {
        let mut s = state(vec![], vec!["a"]);
        // entries: 0 hdr, 1 note, 2 hdr, 3 Registry0, 4 hdr, 5 Create
        s.selection.set(Some(5));
        assert_eq!(s.total_rows(), 6);
        assert_eq!(s.row_at(5), Some(LabelRow::Create));
        s.selection.set(Some(5));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::CreateLabel
        );
        // Creating is reserved for Enter/space on the create row: `n` no
        // longer creates, and `e` on the create row is a no-op.
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('n')),
            LabelAction::None
        );
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::None
        );
    }

    #[test]
    fn resolve_e_edits_registry_label_only() {
        let mut s = state(vec!["bug"], vec!["bug", "design"]);
        // `e` on the applied row does nothing; editing belongs to the Labels
        // section.
        s.selection.set(Some(1)); // flat 1 = Applied0 ("bug")
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::None
        );
        // `e` on a registry label edits it.
        s.selection.set(Some(4)); // flat 4 = Registry1 ("design")
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::EditLabel("design".into())
        );
    }

    #[test]
    fn navigation_skips_headers_and_clamps_to_bounds() {
        let mut s = state(vec![], vec!["a", "b"]);
        // entries: 0 hdr, 1 note, 2 hdr, 3 Registry0, 4 Registry1, 5 hdr, 6 Create
        assert_eq!(s.first_selectable(), Some(3));
        s.selection.set(Some(3));
        s.navigate_up();
        assert_eq!(s.selection.get(), Some(3), "up at top stays at top");

        s.navigate_down();
        assert_eq!(s.selection.get(), Some(4), "down moves to next selectable");
        s.selection.set(Some(6));
        s.navigate_down();
        assert_eq!(s.selection.get(), Some(6), "down at bottom stays at bottom");
    }

    #[test]
    fn navigation_skips_over_header_rows() {
        // applied has items, so between the last applied row and the first
        // registry row there is a "Labels" header that must be skipped.
        let mut s = state(vec!["bug"], vec!["bug", "design"]);
        s.selection.set(Some(1)); // flat 1 = Applied0 (last applied row)
        s.navigate_down();
        assert_eq!(
            s.selection.get(),
            Some(3),
            "down from the last applied row skips the header to the first registry row"
        );
        assert_eq!(s.row_at(3), Some(LabelRow::Registry(0)));
    }

    #[test]
    fn resolve_unhandled_key_is_noop() {
        let mut s = state(vec!["a"], vec!["a"]);
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('q')),
            LabelAction::None
        );
    }

    #[test]
    fn header_rows_are_not_selectable_but_items_are() {
        let s = state(vec!["bug"], vec!["bug"]);
        // entries: 0 Header, 1 Applied0, 2 Header, 3 Registry0, 4 Header, 5 Create
        assert!(!is_selectable(s.row_at(0).unwrap()), "index 0 is a header");
        assert!(
            is_selectable(s.row_at(1).unwrap()),
            "index 1 is an applied row"
        );
        assert!(!is_selectable(s.row_at(2).unwrap()), "index 2 is a header");
        assert!(!is_selectable(s.row_at(4).unwrap()), "index 4 is a header");
        assert!(
            is_selectable(s.row_at(5).unwrap()),
            "index 5 is the create row"
        );
    }

    #[test]
    fn empty_sections_show_non_selectable_notes() {
        let s = state(vec![], vec![]);
        // entries: 0 Header, 1 Note("(none)"), 2 Header, 3 Note, 4 Header, 5 Create
        assert_eq!(s.row_at(1), Some(LabelRow::Note("(none)")));
        assert!(!is_selectable(s.row_at(1).unwrap()));
        assert_eq!(s.row_at(3), Some(LabelRow::Note("(none defined yet)")));
        assert!(!is_selectable(s.row_at(3).unwrap()));
    }

    fn tags(names: Vec<&str>) -> Vec<String> {
        names.into_iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn intersect_tags_single_card_is_its_own_tags() {
        assert_eq!(
            intersect_tags(&[tags(vec!["bug", "p1"])]),
            vec!["bug".to_string(), "p1".to_string()]
        );
    }

    #[test]
    fn intersect_tags_empty_input_is_empty() {
        assert!(intersect_tags(&[]).is_empty());
    }

    #[test]
    fn intersect_tags_multi_card_keeps_only_shared_in_first_order() {
        // 'bug' is on all three -> kept, in card A's order; the other tags on
        // card A are missing from at least one card -> dropped.
        let result = intersect_tags(&[
            tags(vec!["bug", "p1", "design"]),
            tags(vec!["ux", "bug"]),
            tags(vec!["bug"]),
        ]);
        assert_eq!(
            result,
            vec!["bug".to_string()],
            "only 'bug' is on all three, in card A's order"
        );
    }

    #[test]
    fn intersect_tags_multi_shared_preserves_first_card_order() {
        // 'a' and 'c' are on all three; 'b' is not. Expect them in card A's
        // order, not card B's.
        let result = intersect_tags(&[
            tags(vec!["a", "b", "c"]),
            tags(vec!["c", "a", "d"]),
            tags(vec!["a", "c"]),
        ]);
        assert_eq!(result, vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn intersect_tags_no_shared_is_empty() {
        assert!(
            intersect_tags(&[tags(vec!["bug"]), tags(vec!["p1"])]).is_empty(),
            "disjoint tags share nothing"
        );
    }

    #[test]
    fn intersect_tags_card_with_no_tags_shares_nothing() {
        assert!(
            intersect_tags(&[tags(vec!["bug"]), tags(vec![])]).is_empty(),
            "a card with no tags shares nothing"
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
            .map(|i| i < s.total_rows() && is_selectable(s.row_at(i).unwrap()))
            .unwrap_or(false));
    }
}
