//! State for the "Manage Labels" card dialog.
//!
//! The dialog is a single flat list (the same pattern as the sprint picker and
//! the relationship lists): one scrollable "Labels" box holding every label,
//! plus the "New label" action row. The labels are internally divided by a
//! non-selectable [`LabelRow::Divider`]: labels that are on at **least one** of
//! the managed cards sit above it, labels on **none** sit below. Each label
//! row carries one of three applied marks, derived from how many of the managed
//! cards carry the label:
//!
//! * `[✓]` — on **every** selected card;
//! * `[-]` — on **some but not all** (the partial / multiselect edge case,
//!   shown with the keys of the cards it is on);
//! * `[ ]` — on **none** of the selected cards.
//!
//! With a single managed card the partial state is unreachable, so the dialog
//! collapses to the ordinary on-card / not-on-card toggle. [`LabelManagerState`]
//! keeps the managed cards (identifier + their tags) and a computed
//! [`LabelItem`] per label; [`LabelManagerState::entries`] is the source of
//! truth for the flat ordering, and [`LabelManagerState::resolve`] maps a key
//! to a semantic [`LabelAction`], so the apply/remove/delete/create decision
//! matrix stays pure and unit-testable.
use kanban_core::SelectionState;
use kanban_domain::Label;
use std::collections::HashSet;
use uuid::Uuid;

/// Which row the flat cursor is pointing at.
///
/// [`LabelRow::Label`], [`LabelRow::Create`] are selectable; [`LabelRow::Divider`]
/// and [`LabelRow::Note`] are not (the cursor skims over them). A `Label` row is
/// a single entry in [`LabelManagerState::rows`], which unifies global-registry
/// labels and stray tags (tags on a card with no registry definition) and is
/// independent of applied state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelRow {
    /// A label row: a global-registry label or a stray tag. Index into
    /// [`LabelManagerState::rows`].
    Label(usize),
    /// The internal divider between "on at least one card" (above) and "on
    /// none" (below). Non-selectable layout element.
    Divider,
    /// Muted note shown when the label list is empty (e.g. "(no labels)").
    Note(&'static str),
    /// The "new label" input row at the bottom of the list.
    Create,
}

/// The semantic result of a key press in the dialog. Drives the handler that
/// turns it into a service call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelAction {
    /// No-op key (anything unhandled).
    None,
    /// Remove this label from the managed card(s).
    RemoveFromCard(String),
    /// Add this label to the managed card(s) (auto-registers if missing).
    ApplyToCard(String),
    /// Delete this label from the registry (and, if applied, from the cards).
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

/// A managed card reduced to what the dialog needs: its human-readable key
/// (e.g. `KAN-12`, used for the partial-state listing) and its current tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedCard {
    /// Human-readable identifier, e.g. `KAN-12`.
    pub key: String,
    /// The card's tags (label names), in the card's own order.
    pub tags: Vec<String>,
}

/// One computed label entry for the dialog list. A label is "on" a card when
/// that card's `tags` contain this entry's `name`; see
/// [`LabelManagerState::applied_card_keys`] for the per-card membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelItem {
    /// The label name (tag string).
    pub name: String,
    /// Whether the label has a global registry definition. `false` for stray
    /// tags, which then disable `x` (delete) and `e` (edit).
    pub defined: bool,
    /// The registry description, when the label is defined.
    pub description: Option<String>,
}

/// Flat-list cursor state for [`super::DialogMode::ManageLabels`].
pub struct LabelManagerState {
    /// The ids of the managed cards. Usually one (opened from a card's detail
    /// view) but several when opened from the card list with multi-select
    /// active. Changes apply to all of them.
    pub card_ids: Vec<Uuid>,
    /// The managed cards (identifier + tags), aligned 1:1 with [`Self::card_ids`].
    /// Drives the per-label applied state and the partial listing.
    pub cards: Vec<ManagedCard>,
    /// The computed label entries, ordered "on at least one" first (above the
    /// divider), "on none" next, each group in a stable order.
    pub rows: Vec<LabelItem>,
    /// Shared cursor over the flat [`Self::entries`] list.
    pub selection: SelectionState,
    /// `true` while the cursor is on the Create row and in active-typing
    /// (insert) mode: red border, typed chars go into the input buffer,
    /// and ESC returns to normal mode instead of closing the dialog.
    /// Set to `true` when the cursor arrives at the Create row via navigation,
    /// or when a character is typed from normal mode. Set to `false` when
    /// ESC is pressed in insert mode, when the cursor navigates away, or
    /// after confirming (creating a label).
    pub create_insert: bool,
}

impl Default for LabelManagerState {
    fn default() -> Self {
        Self {
            card_ids: Vec::new(),
            cards: Vec::new(),
            rows: Vec::new(),
            selection: SelectionState::new(),
            create_insert: false,
        }
    }
}

fn is_selectable(row: LabelRow) -> bool {
    matches!(row, LabelRow::Label(_) | LabelRow::Create)
}

impl LabelManagerState {
    /// The managed cards that carry a given label, in managed-card order. Used
    /// to decide the row's applied mark and to build the partial listing.
    pub fn applied_card_keys(&self, name: &str) -> Vec<String> {
        self.cards
            .iter()
            .filter(|c| c.tags.iter().any(|t| t == name))
            .map(|c| c.key.clone())
            .collect()
    }

    /// Whether a label is on every managed card (and at least one exists).
    pub fn is_on_all_cards(&self, name: &str) -> bool {
        !self.cards.is_empty() && self.cards.iter().all(|c| c.tags.iter().any(|t| t == name))
    }

    /// The flat row list in layout order: the label rows (on-at-least-one first,
    /// then a [`LabelRow::Divider`], then on-none), or a "(no labels)" note
    /// when there is nothing; then the `Create` row.
    pub fn entries(&self) -> Vec<LabelRow> {
        let mut e = Vec::new();
        if self.rows.is_empty() {
            e.push(LabelRow::Note("(no labels)"));
        } else {
            let on_some = self
                .rows
                .iter()
                .take_while(|r| !self.applied_card_keys(&r.name).is_empty())
                .count();
            for i in 0..self.rows.len() {
                e.push(LabelRow::Label(i));
                if i + 1 == on_some && on_some > 0 && on_some < self.rows.len() {
                    e.push(LabelRow::Divider);
                }
            }
        }
        e.push(LabelRow::Create);
        e
    }

    /// Total number of flat rows (labels, divider, notes, create).
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

    /// The index into [`Self::rows`] if the cursor is on a label row.
    fn current_label(&self) -> Option<usize> {
        match self.current_row() {
            Some(LabelRow::Label(i)) => Some(i),
            _ => None,
        }
    }

    /// The first selectable flat index.
    pub fn first_selectable(&self) -> Option<usize> {
        let entries = self.entries();
        kanban_view::list_nav::first_selectable_index(entries.len(), |i| is_selectable(entries[i]))
    }

    /// Move the cursor to the next selectable row, skimming over dividers and
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

    /// Move the cursor to the previous selectable row, skimming over dividers
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

    /// Build the computed label entries from the managed cards and the global
    /// registry, then re-pin the cursor.
    ///
    /// The entry set is the registry's labels plus any tag on a managed card
    /// that has no registry definition (so it can still be shown and removed).
    /// Entries are ordered "on at least one card" first (registry order, then
    /// stray tags), then "on none of the cards" (registry order).
    pub fn refresh(&mut self, cards: Vec<ManagedCard>, registry: Vec<Label>) {
        let mut defined_applied: Vec<LabelItem> = Vec::new();
        let mut defined_not: Vec<LabelItem> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for label in &registry {
            seen.insert(label.name.clone());
            let item = LabelItem {
                name: label.name.clone(),
                defined: true,
                description: label.description.clone(),
            };
            let on_some = cards
                .iter()
                .any(|c| c.tags.iter().any(|t| t == &label.name));
            if on_some {
                defined_applied.push(item);
            } else {
                defined_not.push(item);
            }
        }
        let mut strays: Vec<LabelItem> = Vec::new();
        for card in &cards {
            for tag in &card.tags {
                if seen.contains(tag) {
                    continue;
                }
                seen.insert(tag.clone());
                // A stray tag is on at least one card (this one is), so it goes
                // in the applied group; x/e are disabled via `defined == false`.
                strays.push(LabelItem {
                    name: tag.clone(),
                    defined: false,
                    description: None,
                });
            }
        }
        let mut rows = Vec::new();
        rows.append(&mut defined_applied);
        rows.append(&mut strays);
        rows.append(&mut defined_not);
        self.cards = cards;
        self.rows = rows;
        let keep = self
            .selection
            .get()
            .filter(|i| self.entries().get(*i).copied().is_some_and(is_selectable));
        self.selection.set(keep.or_else(|| self.first_selectable()));
        // If the cursor lands on the Create row, start in insert mode (red)
        // so the user can immediately type a name without an extra key press.
        self.create_insert =
            self.selection.get().and_then(|i| self.row_at(i)) == Some(LabelRow::Create);
    }

    /// Map a key press to the action the cursor row demands. `j`/`k` move the
    /// cursor in-place, so the returned [`LabelAction::NavigateDown`]/`NavigateUp`
    /// arms are informational no-ops in the handler.
    ///
    /// Enter/space converges the set to a consistent state: a label already on
    /// **every** managed card is removed from all of them; a label on **some or
    /// none** is applied to all (filling the gaps). For a single card this is
    /// the ordinary apply/remove toggle.
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
            // `e` edits a label that has a global definition. Stray tags have no
            // registry definition to edit, so `e` is a no-op on them.
            KeyCode::Char('e') => match self.current_label() {
                Some(i) if self.rows[i].defined => {
                    LabelAction::EditLabel(self.rows[i].name.clone())
                }
                _ => LabelAction::None,
            },
            // `x` deletes a label that has a global definition. Stray tags have
            // no registry entry to delete (removing them from the cards is
            // Enter's job), so `x` is a no-op on them.
            KeyCode::Char('x') => match self.current_label() {
                Some(i) if self.rows[i].defined => {
                    LabelAction::DeleteLabel(self.rows[i].name.clone())
                }
                _ => LabelAction::None,
            },
            KeyCode::Enter | KeyCode::Char(' ') => match self.current_row() {
                Some(LabelRow::Label(i)) => {
                    let name = self.rows[i].name.clone();
                    if self.is_on_all_cards(&name) {
                        LabelAction::RemoveFromCard(name)
                    } else {
                        LabelAction::ApplyToCard(name)
                    }
                }
                Some(LabelRow::Create) => LabelAction::CreateLabel,
                Some(LabelRow::Divider) | Some(LabelRow::Note(_)) | None => LabelAction::None,
            },
            _ => LabelAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str, tags: Vec<&str>) -> ManagedCard {
        ManagedCard {
            key: key.to_string(),
            tags: tags.into_iter().map(|t| t.to_string()).collect(),
        }
    }

    fn make(cards: Vec<ManagedCard>, registry: Vec<&str>) -> LabelManagerState {
        let mut s = LabelManagerState::default();
        s.refresh(
            cards,
            registry.into_iter().map(|n| Label::named(n)).collect(),
        );
        s
    }

    #[test]
    fn single_card_binary_applied_and_not_applied_groups() {
        // KAN-1 has "design"; registry is design + p1.
        let s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        // Rows ordered on-some first: design (applied), then p1 (not).
        assert_eq!(
            s.rows.iter().map(|r| r.name.clone()).collect::<Vec<_>>(),
            vec!["design", "p1"]
        );
        // Flat layout: Label(design), Divider, Label(p1), Create.
        assert_eq!(s.row_at(0), Some(LabelRow::Label(0)));
        assert_eq!(s.row_at(1), Some(LabelRow::Divider));
        assert_eq!(s.row_at(2), Some(LabelRow::Label(1)));
        assert_eq!(s.row_at(3), Some(LabelRow::Create));

        // The first selectable row is design (the applied one).
        let mut s = s;
        s.refresh(
            vec![card("KAN-1", vec!["design"])],
            vec![Label::named("design"), Label::named("p1")],
        );
        assert_eq!(s.current_row(), Some(LabelRow::Label(0)));
    }

    #[test]
    fn enter_on_card_applied_label_removes_it() {
        let mut s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        s.selection.set(Some(0)); // design, on all cards here (single card with it)
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::RemoveFromCard("design".into())
        );
    }

    #[test]
    fn enter_on_not_applied_label_applies_it() {
        let mut s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        s.selection.set(Some(2)); // p1, on none
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::ApplyToCard("p1".into())
        );
    }

    #[test]
    fn multi_card_apply_to_all_on_partial_and_remove_on_full() {
        // "all" is on every card; "some" is on two of three; "none" on zero.
        let cards = vec![
            card("KAN-12", vec!["all", "some"]),
            card("KAN-13", vec!["all", "some"]),
            card("KAN-17", vec!["all"]),
        ];
        let mut s = make(cards, vec!["all", "some", "none"]);
        assert_eq!(
            s.applied_card_keys("all"),
            vec!["KAN-12", "KAN-13", "KAN-17"]
        );
        assert_eq!(s.applied_card_keys("some"), vec!["KAN-12", "KAN-13"]);
        assert!(s.applied_card_keys("none").is_empty());
        assert!(s.is_on_all_cards("all"), "all is on every card");
        assert!(!s.is_on_all_cards("some"), "some is on only two");

        // On-all label: Enter removes it from the set.
        s.selection.set(Some(0)); // "all"
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::RemoveFromCard("all".into())
        );
        // Partial label: Enter applies it to all (fills KAN-17).
        s.selection.set(Some(1)); // "some"
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::ApplyToCard("some".into())
        );
        // Zero label: Enter applies it to all.
        s.selection.set(Some(3)); // "none" (after the divider at 2)
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::ApplyToCard("none".into())
        );
    }

    #[test]
    fn stray_tag_applied_group_but_x_and_e_are_noops() {
        // KAN-1 carries "stray", which has no registry definition; registry is empty.
        let mut s = make(vec![card("KAN-1", vec!["stray"])], vec![]);
        assert_eq!(s.rows.len(), 1);
        assert!(!s.rows[0].defined);
        // Single card with the tag -> on-all -> Enter removes it.
        s.selection.set(Some(0));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Enter),
            LabelAction::RemoveFromCard("stray".into())
        );
        // x and e need a registry definition, so they are no-ops on the stray.
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('x')),
            LabelAction::None,
            "x must be a no-op on a stray tag"
        );
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::None,
            "e must be a no-op on a stray tag"
        );
    }

    #[test]
    fn defined_label_x_deletes_and_e_edits() {
        let mut s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        s.selection.set(Some(0)); // design (defined)
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('x')),
            LabelAction::DeleteLabel("design".into())
        );
        s.selection.set(Some(2)); // p1 (defined)
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('e')),
            LabelAction::EditLabel("p1".into())
        );
    }

    #[test]
    fn no_divider_when_all_or_none_are_applied() {
        // All labels applied -> every row is above the (absent) divider.
        let s = make(vec![card("KAN-1", vec!["a"])], vec!["a"]);
        assert!(
            s.entries().iter().all(|r| !matches!(r, LabelRow::Divider)),
            "no divider when the set is fully applied"
        );
        // No label applied -> no divider either.
        let s2 = make(vec![card("KAN-1", vec![])], vec!["a", "b"]);
        assert!(
            s2.entries().iter().all(|r| !matches!(r, LabelRow::Divider)),
            "no divider when nothing is applied"
        );
    }

    #[test]
    fn divider_is_not_selectable_but_labels_are() {
        let s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        assert!(
            is_selectable(s.row_at(0).unwrap()),
            "design row is selectable"
        );
        assert!(
            !is_selectable(s.row_at(1).unwrap()),
            "divider is not selectable"
        );
        assert!(is_selectable(s.row_at(2).unwrap()), "p1 row is selectable");
        assert!(
            is_selectable(s.row_at(3).unwrap()),
            "create row is selectable"
        );
    }

    #[test]
    fn navigation_skips_the_divider() {
        // Layout: Label(design), Divider, Label(p1), Create.
        let mut s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        assert_eq!(s.first_selectable(), Some(0));
        s.selection.set(Some(0));
        s.navigate_down();
        assert_eq!(
            s.selection.get(),
            Some(2),
            "down from design must skip the divider to p1"
        );
        assert_eq!(s.row_at(2), Some(LabelRow::Label(1)));
        s.navigate_up();
        assert_eq!(
            s.selection.get(),
            Some(0),
            "up from p1 must skip the divider to design"
        );
    }

    #[test]
    fn refresh_pins_cursor_to_first_selectable() {
        let mut s = make(vec![card("KAN-1", vec!["design"])], vec!["design", "p1"]);
        s.selection.set(Some(3)); // the Create row
        s.refresh(
            vec![card("KAN-2", vec!["design", "p1"])],
            vec![Label::named("design"), Label::named("p1")],
        );
        // After the refresh the first selectable is the first applied label.
        assert!(s
            .selection
            .get()
            .map(|i| i < s.total_rows() && is_selectable(s.row_at(i).unwrap()))
            .unwrap_or(false));
    }

    #[test]
    fn resolve_unhandled_key_is_noop() {
        let mut s = make(vec![card("KAN-1", vec!["design"])], vec!["design"]);
        s.selection.set(Some(0));
        assert_eq!(
            s.resolve(crossterm::event::KeyCode::Char('q')),
            LabelAction::None
        );
    }
}
