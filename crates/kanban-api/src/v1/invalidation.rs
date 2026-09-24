use kanban_domain::{EntityIds, Invalidation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Ids are sorted before serialization so the wire form is stable across runs
/// despite `HashSet` iteration order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityIdsDto {
    #[serde(default)]
    pub boards: Vec<Uuid>,
    #[serde(default)]
    pub columns: Vec<Uuid>,
    #[serde(default)]
    pub cards: Vec<Uuid>,
    #[serde(default)]
    pub sprints: Vec<Uuid>,
    #[serde(default)]
    pub graph: bool,
    #[serde(default)]
    pub prefixes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "entities")]
pub enum InvalidationDto {
    All,
    Entities(EntityIdsDto),
}

impl From<&EntityIds> for EntityIdsDto {
    fn from(value: &EntityIds) -> Self {
        let mut boards: Vec<Uuid> = value.boards.iter().copied().collect();
        let mut columns: Vec<Uuid> = value.columns.iter().copied().collect();
        let mut cards: Vec<Uuid> = value.cards.iter().copied().collect();
        let mut sprints: Vec<Uuid> = value.sprints.iter().copied().collect();
        boards.sort();
        columns.sort();
        cards.sort();
        sprints.sort();
        Self {
            boards,
            columns,
            cards,
            sprints,
            graph: value.graph,
            prefixes: value.prefixes,
        }
    }
}

impl From<&EntityIdsDto> for EntityIds {
    fn from(value: &EntityIdsDto) -> Self {
        Self {
            boards: value.boards.iter().copied().collect(),
            columns: value.columns.iter().copied().collect(),
            cards: value.cards.iter().copied().collect(),
            sprints: value.sprints.iter().copied().collect(),
            graph: value.graph,
            prefixes: value.prefixes,
        }
    }
}

impl From<&Invalidation> for InvalidationDto {
    fn from(value: &Invalidation) -> Self {
        match value {
            Invalidation::All => Self::All,
            Invalidation::Entities(ids) => Self::Entities(ids.into()),
        }
    }
}

impl From<&InvalidationDto> for Invalidation {
    fn from(value: &InvalidationDto) -> Self {
        match value {
            InvalidationDto::All => Self::All,
            InvalidationDto::Entities(ids) => Self::Entities(ids.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use kanban_domain::{EntityIds, Invalidation};
    use std::collections::HashSet;
    use uuid::Uuid;

    #[test]
    fn test_invalidation_dto_round_trips_entities_through_json() {
        let ids = EntityIds {
            boards: HashSet::from([Uuid::new_v4()]),
            columns: HashSet::from([Uuid::new_v4(), Uuid::new_v4()]),
            cards: HashSet::from([Uuid::new_v4(), Uuid::new_v4()]),
            sprints: HashSet::from([Uuid::new_v4()]),
            graph: true,
            prefixes: true,
        };
        let invalidation = Invalidation::Entities(ids);
        let dto = InvalidationDto::from(&invalidation);
        let json = serde_json::to_string(&dto).unwrap();
        let parsed: InvalidationDto = serde_json::from_str(&json).unwrap();
        let round_tripped: Invalidation = (&parsed).into();
        assert_eq!(round_tripped, invalidation);
    }

    #[test]
    fn test_invalidation_dto_all_serializes_as_an_explicit_marker() {
        let dto = InvalidationDto::from(&Invalidation::All);
        let value = serde_json::to_value(&dto).unwrap();
        assert_eq!(value["scope"], "all");
        assert!(value.get("entities").is_none());
        let parsed: InvalidationDto = serde_json::from_value(value).unwrap();
        let round_tripped: Invalidation = (&parsed).into();
        assert_eq!(round_tripped, Invalidation::All);
    }

    #[test]
    fn test_invalidation_dto_ids_serialize_in_a_stable_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        let ids_one = EntityIds {
            cards: HashSet::from([a, b, c]),
            ..Default::default()
        };
        let ids_two = EntityIds {
            cards: HashSet::from([c, a, b]),
            ..Default::default()
        };

        let dto_one = InvalidationDto::from(&Invalidation::Entities(ids_one));
        let dto_two = InvalidationDto::from(&Invalidation::Entities(ids_two));

        let json_one = serde_json::to_string(&dto_one).unwrap();
        let json_two = serde_json::to_string(&dto_two).unwrap();
        assert_eq!(json_one, json_two);
    }
}
