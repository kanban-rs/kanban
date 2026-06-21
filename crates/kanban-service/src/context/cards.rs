use super::KanbanContext;
use kanban_domain::commands::{CardCommand, Command};
use kanban_domain::{
    ArchivedCard, Card, CardListFilter, CardSummary, CardUpdate, Column, KanbanError, KanbanResult,
    Sprint,
};
use uuid::Uuid;

impl KanbanContext {
    pub(super) fn create_card_impl(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: kanban_domain::CreateCardOptions,
    ) -> KanbanResult<Card> {
        use kanban_domain::commands::CreateCard;
        let position = self.backend.list_cards_by_column(column_id)?.len() as i32;
        let card_number = self
            .backend
            .get_board(board_id)?
            .map(|b| b.card_counter)
            .unwrap_or(1);
        let id = Uuid::new_v4();
        let cmd = Command::Card(CardCommand::Create(CreateCard {
            id,
            card_number,
            board_id,
            column_id,
            title,
            position,
            options,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Card creation succeeded but card not found".into())
        })
    }

    pub(super) fn list_cards_impl(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        let cards = self.filter_cards(&filter)?;
        Ok(cards.iter().map(CardSummary::from).collect())
    }

    pub(super) fn get_card_impl(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.backend.get_card(id)
    }

    pub(super) fn find_cards_by_identifier_impl(
        &self,
        identifier: &str,
    ) -> KanbanResult<Vec<Card>> {
        use kanban_domain::search::find_cards_by_identifier as search;
        let cards = self.backend.list_all_cards()?;
        let columns = self.backend.list_all_columns()?;
        let boards = self.backend.list_boards()?;
        let sprints = self.backend.list_all_sprints()?;
        Ok(search(identifier, &cards, &columns, &boards, &sprints)
            .into_iter()
            .cloned()
            .collect())
    }

    pub(super) fn list_all_cards_impl(&self) -> KanbanResult<Vec<Card>> {
        self.backend.list_all_cards()
    }

    pub(super) fn list_all_columns_impl(&self) -> KanbanResult<Vec<Column>> {
        self.backend.list_all_columns()
    }

    pub(super) fn list_all_sprints_impl(&self) -> KanbanResult<Vec<Sprint>> {
        self.backend.list_all_sprints()
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
            None => self.backend.list_cards_by_column(column_id)?.len() as i32,
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
        let archived = self
            .backend
            .get_archived_card(id)?
            .ok_or_else(|| KanbanError::not_found("archived card", id))?;

        let target_column = if let Some(col_id) = column_id {
            if self.backend.get_column(col_id)?.is_none() {
                return Err(KanbanError::not_found("Column", col_id));
            }
            col_id
        } else if self
            .backend
            .get_column(archived.original_column_id)?
            .is_some()
        {
            archived.original_column_id
        } else {
            return Err(KanbanError::validation("Original column no longer exists. Specify --column-id to restore to a different column"));
        };

        let position = archived.original_position;
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
