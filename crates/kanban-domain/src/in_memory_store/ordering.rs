use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Board, Card, Column};

pub(super) trait PositionOrdered {
    fn position(&self) -> i32;
    fn created_at(&self) -> DateTime<Utc>;
    fn id(&self) -> Uuid;
}

pub(super) fn sort_by_position<T: PositionOrdered>(items: &mut [T]) {
    items.sort_by(|a, b| {
        a.position()
            .cmp(&b.position())
            .then_with(|| a.created_at().cmp(&b.created_at()))
            .then_with(|| a.id().cmp(&b.id()))
    });
}

impl PositionOrdered for Board {
    fn position(&self) -> i32 {
        self.position
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    fn id(&self) -> Uuid {
        self.id
    }
}

impl PositionOrdered for Column {
    fn position(&self) -> i32 {
        self.position
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    fn id(&self) -> Uuid {
        self.id
    }
}

impl PositionOrdered for Card {
    fn position(&self) -> i32 {
        self.position
    }
    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    fn id(&self) -> Uuid {
        self.id
    }
}
