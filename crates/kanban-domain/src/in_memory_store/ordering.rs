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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Item {
        position: i32,
        created_at: DateTime<Utc>,
        id: Uuid,
    }

    impl PositionOrdered for Item {
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

    fn at(secs: i64) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn test_sort_by_position_orders_by_position_first() {
        let mut items = vec![
            Item {
                position: 2,
                created_at: at(0),
                id: Uuid::nil(),
            },
            Item {
                position: 0,
                created_at: at(0),
                id: Uuid::nil(),
            },
            Item {
                position: 1,
                created_at: at(0),
                id: Uuid::nil(),
            },
        ];
        sort_by_position(&mut items);
        assert_eq!(
            items.iter().map(|i| i.position).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn test_sort_by_position_breaks_equal_position_by_created_at() {
        let mut items = vec![
            Item {
                position: 0,
                created_at: at(2_000),
                id: Uuid::nil(),
            },
            Item {
                position: 0,
                created_at: at(1_000),
                id: Uuid::nil(),
            },
        ];
        sort_by_position(&mut items);
        assert_eq!(
            items.iter().map(|i| i.created_at).collect::<Vec<_>>(),
            vec![at(1_000), at(2_000)],
            "equal position must resolve by created_at ascending"
        );
    }

    #[test]
    fn test_sort_by_position_breaks_equal_position_and_time_by_id() {
        let low = Uuid::from_u128(1);
        let high = Uuid::from_u128(2);
        let mut items = vec![
            Item {
                position: 0,
                created_at: at(0),
                id: high,
            },
            Item {
                position: 0,
                created_at: at(0),
                id: low,
            },
        ];
        sort_by_position(&mut items);
        assert_eq!(
            items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![low, high],
            "equal position and created_at must resolve by id for a total order"
        );
    }
}
