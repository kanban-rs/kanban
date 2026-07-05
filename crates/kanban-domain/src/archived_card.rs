use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

use crate::{
    card::{Card, CardSummary},
    column::ColumnId,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedCard {
    #[serde(with = "crate::card_factory::card_serde")]
    pub card: Card,
    pub archived_at: DateTime<Utc>,
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

impl ArchivedCard {
    pub fn new(card: Card, original_column_id: ColumnId, original_position: i32) -> Self {
        Self {
            card,
            archived_at: Utc::now(),
            original_column_id,
            original_position,
        }
    }

    pub fn into_card(self) -> Card {
        self.card
    }

    pub fn card_ref(&self) -> &Card {
        &self.card
    }

    pub fn card_mut(&mut self) -> &mut Card {
        &mut self.card
    }
}

impl From<ArchivedCard> for Card {
    fn from(archived_card: ArchivedCard) -> Self {
        archived_card.card
    }
}

impl Borrow<Card> for ArchivedCard {
    fn borrow(&self) -> &Card {
        &self.card
    }
}

impl crate::archival::ArchivedEntity for ArchivedCard {
    fn entity_id(&self) -> uuid::Uuid {
        self.card.id
    }

    fn metadata(&self) -> crate::archival::ArchiveMetadata {
        crate::archival::ArchiveMetadata::at(self.archived_at)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedCardSummary {
    pub card: CardSummary,
    pub archived_at: DateTime<Utc>,
    pub original_column_id: ColumnId,
    pub original_position: i32,
}

impl From<&ArchivedCard> for ArchivedCardSummary {
    fn from(a: &ArchivedCard) -> Self {
        Self {
            card: CardSummary::from(&a.card),
            archived_at: a.archived_at,
            original_column_id: a.original_column_id,
            original_position: a.original_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archival::ArchivedEntity;
    use crate::{board::Board, card::Card, column::Column};

    #[test]
    fn test_archived_card_implements_archived_entity() {
        // Built via public constructors (the in-memory `test_support` module is
        // private and not usable from here).
        let mut board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, col.id, "T", 0);
        let card_id = card.id;

        let ac = ArchivedCard::new(card, col.id, 0);
        let field_archived_at = ac.archived_at;

        assert_eq!(ArchivedEntity::entity_id(&ac), card_id);
        // The trait method reflects the record's own `archived_at` field.
        assert_eq!(ac.archived_at(), field_archived_at);
    }
}
