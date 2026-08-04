use crate::components::radio_list::{ListItem, RadioList};
use crate::components::sprint_assign_list::render_entry_line;
use chrono::{DateTime, Utc};
use kanban_domain::{Board, Sprint, SprintStatus};
use kanban_view::sprint_assign_list::{
    build_entries, build_entries_active_only, sprint_id_of, SprintAssignEntry,
};
use ratatui::{layout::Rect, Frame};
use uuid::Uuid;

pub struct SprintPickerView<'a> {
    entries: Vec<SprintAssignEntry<'a>>,
    board: &'a Board,
    current_sprint_id: Option<Uuid>,
    initial: Option<usize>,
}

impl<'a> SprintPickerView<'a> {
    pub fn for_card_assignment(
        sprints: &'a [Sprint],
        board: &'a Board,
        current_sprint_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Self {
        let entries = build_entries(sprints, board.id, now);
        let initial = match current_sprint_id {
            Some(id) => entries
                .iter()
                .position(|e| sprint_id_of(e) == Some(id))
                .or(Some(0)),
            None => Some(0),
        };
        Self {
            entries,
            board,
            current_sprint_id,
            initial,
        }
    }

    /// Variant for the create-card picker: hides the Completed/Ended
    /// section entirely (a card being created right now should never
    /// be bound to a sprint that has already finished) and pre-selects
    /// the sole Active non-ended sprint if there is exactly one.
    pub fn for_new_card(sprints: &'a [Sprint], board: &'a Board, now: DateTime<Utc>) -> Self {
        let entries = build_entries_active_only(sprints, board.id, now);
        let active_non_ended: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| match entry {
                SprintAssignEntry::ActiveOrPlanned(s)
                    if s.status == SprintStatus::Active && !s.is_ended(now) =>
                {
                    Some(idx)
                }
                _ => None,
            })
            .collect();
        let initial = if active_non_ended.len() == 1 {
            Some(active_non_ended[0])
        } else {
            Some(0)
        };
        Self {
            entries,
            board,
            current_sprint_id: None,
            initial,
        }
    }

    pub fn initial_selection(&self) -> Option<usize> {
        self.initial
    }

    /// Find the row index of a sprint id (or the "(None)" entry if `id`
    /// is `None`). Returns `None` only when a sprint id is supplied that
    /// does not appear in the current entries — e.g. it was deleted
    /// between frames. Avoids a redundant `build_entries` call for
    /// callers that already have a `SprintPickerView` constructed.
    pub fn index_of_sprint(&self, id: Option<Uuid>) -> Option<usize> {
        match id {
            None => self
                .entries
                .iter()
                .position(|e| matches!(e, SprintAssignEntry::None)),
            Some(target) => self
                .entries
                .iter()
                .position(|e| sprint_id_of(e) == Some(target)),
        }
    }

    pub fn value_at(&self, idx: usize) -> Option<Option<Uuid>> {
        match self.entries.get(idx)? {
            SprintAssignEntry::Header(_) => None,
            SprintAssignEntry::None => Some(None),
            SprintAssignEntry::ActiveOrPlanned(s)
            | SprintAssignEntry::Completed(s)
            | SprintAssignEntry::Ended(s) => Some(Some(s.id)),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, selected: Option<usize>) {
        // Coupled mode: cursor and `[x]` are on the same row.
        self.render_with_cursor(frame, area, selected, selected);
    }

    /// Decoupled render: `checked` is the row that shows `[x]`; `cursor`
    /// is the row that gets the keyboard highlight (and drives scroll +
    /// sticky-header). Callers that want the coupled "cursor IS the
    /// selected row" behaviour pass the same value for both — that is
    /// what `render` does.
    pub fn render_with_cursor(
        &self,
        frame: &mut Frame,
        area: Rect,
        checked: Option<usize>,
        cursor: Option<usize>,
    ) {
        let items: Vec<ListItem<Option<Uuid>>> = self
            .entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let is_checked = checked == Some(idx);
                let is_focused = cursor == Some(idx);
                let label = render_entry_line(
                    entry,
                    is_checked,
                    is_focused,
                    self.current_sprint_id,
                    self.board,
                );
                ListItem {
                    value: sprint_id_of(entry),
                    label,
                    selectable: entry.is_selectable(),
                }
            })
            .collect();

        let list = RadioList::new(&items).with_sticky_header(|items, sel_idx| {
            let upper = sel_idx.min(items.len());
            for i in (0..upper).rev() {
                if !items[i].selectable {
                    return Some((i, items[i].label.clone()));
                }
            }
            None
        });
        list.render(frame, area, cursor);
    }
}
