use super::KanbanContext;
use chrono::{DateTime, Utc};
use kanban_domain::{ArchivedFilter, Card, CardListFilter, KanbanResult};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The set of individually-archived card ids plus an `entity_id -> archived_at`
/// map (read from the archived-card markers).
pub(super) type ArchivedCardIndex = (HashSet<Uuid>, HashMap<Uuid, DateTime<Utc>>);

impl KanbanContext {
    /// The set of individually-archived card ids plus an `entity_id -> archived_at`
    /// map, read once from the archived-card markers. Shared by the selector-aware
    /// gather and by `list_cards_impl`'s `archived_at` stamping.
    pub(super) fn archived_card_index(&self) -> KanbanResult<ArchivedCardIndex> {
        let markers = self.backend.list_archived_cards()?;
        let mut ids = HashSet::with_capacity(markers.len());
        let mut at_by_id = HashMap::with_capacity(markers.len());
        for m in &markers {
            ids.insert(m.entity_id);
            at_by_id.insert(m.entity_id, m.metadata.archived_at);
        }
        Ok((ids, at_by_id))
    }

    pub(super) fn filter_cards(&self, filter: &CardListFilter) -> KanbanResult<Vec<Card>> {
        let (archived_ids, _at) = self.archived_card_index()?;

        // C10a: an explicit `board_id` is a deliberate scoped request, so base the
        // card set on THAT board's own cards (raw) — honoring the board whether it
        // is live or archived. Only the UNSCOPED read stays live-scoped (C3b).
        let (cards, columns, board) = match filter.board_id {
            Some(bid) => {
                let columns = self.backend.list_columns_by_board(bid)?;
                let col_ids: Vec<Uuid> = columns.iter().map(|c| c.id).collect();
                // `list_cards_by_columns` is LIVE-scoped (excludes individually
                // archived cards on both backends), so gather RAW: live cards in
                // the board's columns UNION the board's archived cards (fetched by
                // marker id), then apply the selector.
                let cards = self.gather_board_cards_for_selector(bid, &col_ids, filter.archived)?;
                // `get_board` is unfiltered (reference-marker model): it resolves
                // the head whether the board is live or archived.
                let board = self.backend.get_board(bid)?;
                (cards, columns, board)
            }
            None => (
                self.gather_unscoped_cards_for_selector(&archived_ids, filter.archived)?,
                Vec::new(),
                None,
            ),
        };
        let sprints = match (board.as_ref(), filter.search.as_deref()) {
            (Some(b), Some(q)) if !q.is_empty() => self.backend.list_sprints_by_board(b.id)?,
            _ => Vec::new(),
        };
        // For ArchivedOnly and Include with a board scope, the cards are already
        // pre-scoped by marker board_id (see gather_board_cards_for_selector, which
        // gathers by marker board_id for both selectors). Clear board_id from the
        // downstream filter so filter_and_sort_cards does not re-apply column-
        // membership restriction, which would drop archived cards with deleted columns.
        let effective_filter;
        let filter_ref = if filter.archived != ArchivedFilter::LiveOnly && filter.board_id.is_some()
        {
            effective_filter = CardListFilter {
                board_id: None,
                ..filter.clone()
            };
            &effective_filter
        } else {
            filter
        };
        Ok(kanban_domain::filter_and_sort_cards(
            &cards,
            &columns,
            &sprints,
            board.as_ref(),
            filter_ref,
        ))
    }

    /// Board-scoped raw gather + selector. LiveOnly keeps the pre-selector set
    /// (live cards in the board's columns). ArchivedOnly/Include add the board's
    /// individually-archived cards via marker board_id (KAN-901: uses
    /// list_archived_cards_by_board so cards with deleted columns are not dropped).
    fn gather_board_cards_for_selector(
        &self,
        board_id: Uuid,
        col_ids: &[Uuid],
        selector: ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        let live: Vec<Card> = self.backend.list_cards_by_columns(col_ids)?;
        match selector {
            ArchivedFilter::LiveOnly => Ok(live),
            ArchivedFilter::ArchivedOnly | ArchivedFilter::Include => {
                // Scope by marker board_id (not current column membership) so that
                // archived cards whose original column was deleted are not silently
                // dropped (KAN-901 dangling-column fix).
                let markers = self.backend.list_archived_cards_by_board(board_id)?;
                let mut archived: Vec<Card> = Vec::new();
                for marker in &markers {
                    if let Some(card) = self.backend.get_card(marker.entity_id)? {
                        archived.push(card);
                    }
                }
                if selector == ArchivedFilter::ArchivedOnly {
                    Ok(archived)
                } else {
                    let mut all = live;
                    all.extend(archived);
                    Ok(all)
                }
            }
        }
    }

    /// Unscoped gather + selector, preserving C3b (archived-BOARD descendants stay
    /// excluded regardless of the selector — the selector is about individually
    /// archived CARDS). LiveOnly equals the pre-selector base once C3b is applied:
    /// byte-identical when no board is archived, otherwise the base minus
    /// archived-board descendants.
    fn gather_unscoped_cards_for_selector(
        &self,
        archived_ids: &HashSet<Uuid>,
        selector: ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        let live = self.list_live_cards_impl()?;
        match selector {
            ArchivedFilter::LiveOnly => Ok(live),
            ArchivedFilter::ArchivedOnly | ArchivedFilter::Include => {
                // Individually-archived cards on LIVE boards only: fetch each by
                // marker id and drop those whose column belongs to an archived
                // board (same exclusion `list_live_cards_impl` applies).
                let archived_board_cols = self.archived_board_column_set()?;
                let mut archived: Vec<Card> = Vec::new();
                for id in archived_ids {
                    if let Some(card) = self.backend.get_card(*id)? {
                        if !archived_board_cols.contains(&card.column_id) {
                            archived.push(card);
                        }
                    }
                }
                if selector == ArchivedFilter::ArchivedOnly {
                    Ok(archived)
                } else {
                    let mut all = live;
                    all.extend(archived);
                    Ok(all)
                }
            }
        }
    }

    /// The set of column ids that belong to an ARCHIVED board (used to exclude
    /// archived-board descendants from unscoped reads — C3b).
    fn archived_board_column_set(&self) -> KanbanResult<HashSet<Uuid>> {
        let archived_boards = self.archived_board_id_set()?;
        if archived_boards.is_empty() {
            return Ok(HashSet::new());
        }
        Ok(self
            .backend
            .list_all_columns()?
            .into_iter()
            .filter(|c| archived_boards.contains(&c.board_id))
            .map(|c| c.id)
            .collect())
    }
}
