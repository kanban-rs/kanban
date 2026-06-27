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
