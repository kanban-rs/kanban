use super::KanbanContext;
use kanban_domain::commands::{CardCommand, Command};
use kanban_domain::{
    ArchivedCard, ArchivedEntity, Card, CardListFilter, CardSummary, CardUpdate, Column,
    CreateCardOptions, DomainError, FieldUpdate, KanbanError, KanbanResult, NewCard, Sprint,
};
use uuid::Uuid;

/// Result of an idempotent PUT-create ([`KanbanContext::create_or_replace_card`]):
/// the resulting card plus whether this call created it (`true`, HTTP 201) or
/// replaced an existing one (`false`, HTTP 200). The HTTP binding lives in the
/// server seam; the service tier only reports which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct CardCreateOutcome {
    pub card: Card,
    pub created: bool,
}

impl KanbanContext {
    /// The archival marker's `archived_at` for a card, or `None` if the card is
    /// live. Lets single-entity reads (`get_card`) stamp the archived projection
    /// the same way `list_cards` does, so an archived card is never returned
    /// looking live.
    pub fn card_archived_at(
        &self,
        id: Uuid,
    ) -> KanbanResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(self
            .backend
            .get_archived_card(id)?
            .map(|ac| ac.archived_at()))
    }

    pub fn get_archived_card(&self, id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.backend.get_archived_card(id)
    }

    /// Create a card from a full `NewCard` spec plus an optional client-supplied
    /// id (idempotent PUT-create). The owning board is DERIVED from
    /// `column.board_id` (no `board_id` param). Validates the dual FK: the
    /// `column_id` must exist (missing → `NotFound`), and an optional `sprint_id`
    /// must exist and belong to the derived board (cross-board →
    /// `SprintBoardMismatch`). Resolves the id (client value or a fresh mint) and
    /// enforces uniqueness across BOTH live and archived cards (duplicate →
    /// `AlreadyExists`/409). All validation runs BEFORE the board counter is
    /// minted/bumped, so a rejected create leaves no side effect. `card_number`
    /// minting + board bump stay a service/command-tier responsibility (the
    /// domain `create` is Board-free). Inherent on `KanbanContext` (not a
    /// `KanbanOperations` trait method) — the trait is dual-impl by TUI+CLI and
    /// would force churn there.
    pub fn create_card_from_spec(
        &mut self,
        client_id: Option<Uuid>,
        spec: NewCard,
    ) -> KanbanResult<Card> {
        // FK: column must exist; derive the owning board from it.
        let column = self.require_column(spec.column_id)?;
        let board_id = column.board_id;

        // FK: optional sprint must exist and belong to the derived board.
        if let Some(sprint_id) = spec.sprint_id {
            let sprint = self
                .backend
                .get_sprint(sprint_id)?
                .ok_or_else(|| KanbanError::not_found("Sprint", sprint_id))?;
            if sprint.board_id != board_id {
                return Err(KanbanError::Domain(DomainError::SprintBoardMismatch {
                    sprint_id,
                    sprint_board: sprint.board_id,
                    card_board: board_id,
                }));
            }
        }

        // id uniqueness across live AND archived cards (validate before mint).
        let id = client_id.unwrap_or_else(Uuid::new_v4);
        if self.backend.get_card(id)?.is_some() || self.backend.get_archived_card(id)?.is_some() {
            return Err(KanbanError::already_exists("Card", id));
        }

        let board = self
            .backend
            .get_board(board_id)?
            .ok_or_else(|| KanbanError::not_found("Board", board_id))?;
        let card_number = board.card_counter;
        // Append past the FULL (live + archived) set so a new card shares one
        // coherent ordinal space with any archived siblings (KAN-916 / O1-A).
        let position = self.backend.count_cards_in_column_filtered(
            spec.column_id,
            kanban_domain::ArchivedFilter::Include,
        )? as i32;

        // Keep construction inside the frozen `CreateCard` command (it owns the
        // WIP check, board-counter bump, sprint-log seeding and upserts); the
        // service supplies the minted id/number/position and the rich options.
        let column_id = spec.column_id;
        let cmd = Command::Card(CardCommand::Create(kanban_domain::commands::CreateCard {
            id,
            card_number,
            board_id,
            column_id,
            title: spec.title,
            position,
            options: CreateCardOptions {
                description: spec.description,
                priority: Some(spec.priority),
                points: spec.points,
                due_date: spec.due_date,
                sprint_id: spec.sprint_id,
            },
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Card creation succeeded but card not found".into())
        })
    }

    /// Idempotent PUT-create (create-or-replace) for a card keyed on a
    /// client-supplied `id`: create the card with that id when absent, or fully
    /// replace the content of an existing card with that id. The returned
    /// [`CardCreateOutcome::created`] distinguishes the two so the server seam
    /// can answer 201 vs 200. Server-managed fields (`card_number`, `position`,
    /// `status`, `sprint_logs`) are preserved across the replace arm — only the
    /// content fields carried by `NewCard` are written, and an absent optional
    /// field clears (wholesale replace). The HTTP binding stays in the server
    /// seam.
    pub fn create_or_replace_card(
        &mut self,
        id: Uuid,
        spec: NewCard,
    ) -> KanbanResult<CardCreateOutcome> {
        if self.backend.get_card(id)?.is_none() {
            let card = self.create_card_from_spec(Some(id), spec)?;
            return Ok(CardCreateOutcome {
                card,
                created: true,
            });
        }
        // FK (replace arm): the target column must exist before we dispatch the
        // update — a PUT-replace must not relocate a card to a non-existent
        // column. Routed through the canonical helper (KAN-248).
        self.require_column(spec.column_id)?;
        let card = self.update_card_impl(id, replace_update_from_spec(spec))?;
        Ok(CardCreateOutcome {
            card,
            created: false,
        })
    }

    /// Thin shim over [`create_card_from_spec`](Self::create_card_from_spec)
    /// translating the legacy `CreateCardOptions` create path, so the existing
    /// trait callers do not churn. The service mints the id.
    pub(super) fn create_card_impl(
        &mut self,
        _board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<Card> {
        let spec = NewCard {
            column_id,
            title,
            description: options.description,
            priority: options
                .priority
                .unwrap_or(kanban_domain::CardPriority::Medium),
            due_date: options.due_date,
            points: options.points,
            sprint_id: options.sprint_id,
        };
        self.create_card_from_spec(None, spec)
    }

    pub(super) fn list_cards_impl(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        let (_ids, at_by_id) = self.archived_card_index()?;
        let cards = self.filter_cards(&filter)?;
        Ok(cards
            .iter()
            .map(|c| {
                // Stamp `archived_at` from the marker map; `None` for a live card.
                CardSummary {
                    archived_at: at_by_id.get(&c.id).copied(),
                    ..CardSummary::from(c)
                }
            })
            .collect())
    }

    pub(super) fn get_card_impl(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.backend.get_card(id)
    }

    pub(super) fn find_cards_by_identifier_impl(
        &self,
        identifier: &str,
    ) -> KanbanResult<Vec<Card>> {
        use kanban_domain::search::find_cards_by_identifier as search;
        let cards = self.list_live_cards_impl()?;
        let columns = self.list_live_columns_impl()?;
        let boards = self.backend.list_boards()?;
        let sprints = self.list_live_sprints_impl()?;
        Ok(search(identifier, &cards, &columns, &boards, &sprints)
            .into_iter()
            .cloned()
            .collect())
    }

    /// LIVE-scoped (C3b): the user-facing "list all cards" excludes archived-
    /// board descendants. Raw all-cards is `self.backend.list_all_cards()`.
    pub(super) fn list_all_cards_impl(&self) -> KanbanResult<Vec<Card>> {
        self.list_live_cards_impl()
    }

    pub(super) fn list_all_columns_impl(&self) -> KanbanResult<Vec<Column>> {
        self.list_live_columns_impl()
    }

    pub(super) fn list_all_sprints_impl(&self) -> KanbanResult<Vec<Sprint>> {
        self.list_live_sprints_impl()
    }

    // C3b: canonical LIVE-scoped cross-board reads — exclude descendants of
    // ARCHIVED boards (whose subtree stays in the flat collections). Fidelity
    // paths (snapshot/import/export/migrate) keep reading `self.backend.list_all_*`
    // raw. This is a service-tier filter, uniform across all backends.
    pub(super) fn archived_board_id_set(&self) -> KanbanResult<std::collections::HashSet<Uuid>> {
        Ok(self
            .backend
            .list_archived_boards()?
            .iter()
            .map(|ab| ab.entity_id)
            .collect())
    }
    pub(super) fn list_live_columns_impl(&self) -> KanbanResult<Vec<Column>> {
        let archived = self.archived_board_id_set()?;
        if archived.is_empty() {
            return self.backend.list_all_columns();
        }
        Ok(self
            .backend
            .list_all_columns()?
            .into_iter()
            .filter(|c| !archived.contains(&c.board_id))
            .collect())
    }
    pub(super) fn list_live_sprints_impl(&self) -> KanbanResult<Vec<Sprint>> {
        let archived = self.archived_board_id_set()?;
        if archived.is_empty() {
            return self.backend.list_all_sprints();
        }
        Ok(self
            .backend
            .list_all_sprints()?
            .into_iter()
            .filter(|s| !archived.contains(&s.board_id))
            .collect())
    }
    pub(super) fn list_live_cards_impl(&self) -> KanbanResult<Vec<Card>> {
        let archived = self.archived_board_id_set()?;
        if archived.is_empty() {
            return self.backend.list_all_cards();
        }
        // Exclude ONLY cards whose column belongs to an archived board. Build
        // the (small) archived-column set and drop cards in it — this keeps a
        // card with a dangling/deleted column (an orphan on a LIVE board) as
        // live, matching the pre-C3b behavior for such cards.
        let archived_cols: std::collections::HashSet<Uuid> = self
            .backend
            .list_all_columns()?
            .into_iter()
            .filter(|c| archived.contains(&c.board_id))
            .map(|c| c.id)
            .collect();
        Ok(self
            .backend
            .list_all_cards()?
            .into_iter()
            .filter(|c| !archived_cols.contains(&c.column_id))
            .collect())
    }

    pub(super) fn update_card_impl(&mut self, id: Uuid, updates: CardUpdate) -> KanbanResult<Card> {
        self.update_cards_impl(vec![(id, updates)])?;
        self.get_card_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    pub(super) fn move_card_impl(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<Card> {
        use kanban_domain::commands::{MoveCard, UpdateCard};
        let position = match position {
            Some(p) => p,
            // Append past the FULL (live + archived) set so a moved card — live
            // or archived — lands at a coherent ordinal that never collides with
            // an existing archived sibling (KAN-916 / O1-A). For a destination
            // holding no archived cards this equals the former live-only count.
            None => self
                .backend
                .count_cards_in_column_filtered(column_id, kanban_domain::ArchivedFilter::Include)?
                as i32,
        };
        let mut batch = vec![Command::Card(CardCommand::Move(MoveCard {
            card_id: id,
            new_column_id: column_id,
            new_position: position,
        }))];

        if let Some(new_status) = self.compute_target_status_for_move(id, column_id)? {
            batch.push(Command::Card(CardCommand::Update(UpdateCard {
                card_id: id,
                updates: CardUpdate {
                    status: Some(new_status),
                    ..Default::default()
                },
            })));
        }

        self.execute(batch)?;
        self.get_card_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    pub(super) fn archive_card_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        match self.archive_cards_impl(vec![id]) {
            Ok(0) | Err(KanbanError::Domain(kanban_domain::DomainError::Validation(_))) => {
                Err(KanbanError::not_found("Card", id))
            }
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub(super) fn restore_card_impl(
        &mut self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<Card> {
        use kanban_domain::commands::RestoreCard;
        if self.backend.get_archived_card(id)?.is_none() {
            return Err(KanbanError::not_found("archived card", id));
        }
        // Reference-marker model: the card stayed LIVE in place while archived, so
        // there is no "original column/position" to reconstruct. Restore leaves it
        // where it is unless the caller redirects it to another column.
        let card = self
            .backend
            .get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))?;

        let target_column = if let Some(col_id) = column_id {
            if self.backend.get_column(col_id)?.is_none() {
                return Err(KanbanError::not_found("Column", col_id));
            }
            col_id
        } else {
            // Reference-marker model: the card kept its live column while archived,
            // but that column may have been deleted since. Surface the actionable
            // hint (restored pre-collapse behavior) rather than a bare not_found.
            if self.backend.get_column(card.column_id)?.is_none() {
                return Err(KanbanError::validation(
                    "Original column no longer exists. Specify --column-id to restore to a different column",
                ));
            }
            card.column_id
        };

        let position = card.position;
        let cmd = Command::Card(CardCommand::Restore(RestoreCard {
            card_id: id,
            column_id: target_column,
            position,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    pub(super) fn delete_card_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteCard;
        let cmd = Command::Card(CardCommand::Delete(DeleteCard { card_id: id }));
        self.execute(vec![cmd])
    }

    pub(super) fn list_archived_cards_impl(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.backend.list_archived_cards()
    }

    pub(super) fn list_archived_cards_by_board_impl(
        &self,
        board_id: Uuid,
    ) -> KanbanResult<Vec<ArchivedCard>> {
        self.backend.list_archived_cards_by_board(board_id)
    }

    pub(super) fn assign_card_to_sprint_impl(
        &mut self,
        card_id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<Card> {
        self.assign_cards_to_sprint_impl(vec![card_id], sprint_id)?;
        self.get_card_impl(card_id)?
            .ok_or_else(|| KanbanError::not_found("Card", card_id))
    }

    pub(super) fn unassign_card_from_sprint_impl(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        use kanban_domain::commands::UnassignCardFromSprint;
        let cmd = Command::Card(CardCommand::UnassignFromSprint(UnassignCardFromSprint {
            card_id,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card_impl(card_id)?
            .ok_or_else(|| KanbanError::not_found("Card", card_id))
    }

    pub(super) fn get_card_branch_name_impl(&self, id: Uuid) -> KanbanResult<String> {
        let card = self
            .get_card_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))?;
        let column = self
            .backend
            .get_column(card.column_id)?
            .ok_or_else(|| KanbanError::not_found("Column", card.column_id))?;
        let board = self
            .backend
            .get_board(column.board_id)?
            .ok_or_else(|| KanbanError::not_found("Board", column.board_id))?;
        let sprints = self.backend.list_all_sprints()?;
        Ok(card.branch_name(
            &board,
            &sprints,
            self.app_config.effective_default_card_prefix(),
        ))
    }

    pub(super) fn get_card_git_checkout_impl(&self, id: Uuid) -> KanbanResult<String> {
        let card = self
            .get_card_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))?;
        let column = self
            .backend
            .get_column(card.column_id)?
            .ok_or_else(|| KanbanError::not_found("Column", card.column_id))?;
        let board = self
            .backend
            .get_board(column.board_id)?
            .ok_or_else(|| KanbanError::not_found("Board", column.board_id))?;
        let sprints = self.backend.list_all_sprints()?;
        Ok(card.git_checkout_command(
            &board,
            &sprints,
            self.app_config.effective_default_card_prefix(),
        ))
    }
}

/// Map a `NewCard` create-spec onto a full-replace `CardUpdate` (the PUT replace
/// arm of [`KanbanContext::create_or_replace_card`]): content fields are set;
/// `Option` fields map to `FieldUpdate` (`Some`→`Set`, `None`→`Clear`, so an
/// absent field is wiped). Server-managed `status`/`position` and the sprint-log
/// history are left untouched (`sprint_id` reassignment runs through its own
/// command), so they are not written here.
fn replace_update_from_spec(spec: NewCard) -> CardUpdate {
    let NewCard {
        column_id,
        title,
        description,
        priority,
        due_date,
        points,
        sprint_id: _,
    } = spec;
    CardUpdate {
        title: Some(title),
        description: match description {
            Some(d) => FieldUpdate::Set(d),
            None => FieldUpdate::Clear,
        },
        priority: Some(priority),
        status: None,
        position: None,
        column_id: Some(column_id),
        due_date: match due_date {
            Some(d) => FieldUpdate::Set(d),
            None => FieldUpdate::Clear,
        },
        points: match points {
            Some(p) => FieldUpdate::Set(p),
            None => FieldUpdate::Clear,
        },
        sprint_id: FieldUpdate::NoChange,
    }
}
