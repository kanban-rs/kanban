use kanban_domain::Prefix;
use serde::{Deserialize, Serialize};

/// Response body for prefix reads. Mirrors the domain row: `name` is the
/// namespace's identity (already normalised), and the two counters are the
/// last card/sprint number minted from it. Read-only -- there is no write
/// route, so `Deserialize` is derived only for round-trip tests / client use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefixResponse {
    pub name: String,
    pub card_counter: u32,
    pub sprint_counter: u32,
}

impl From<&Prefix> for PrefixResponse {
    fn from(p: &Prefix) -> Self {
        let Prefix {
            name,
            card_counter,
            sprint_counter,
        } = p;
        Self {
            name: name.clone(),
            card_counter: *card_counter,
            sprint_counter: *sprint_counter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_response_from_prefix_carries_name_and_counters() {
        let mut prefix = Prefix::new("kan");
        prefix.card_counter = 42;
        prefix.sprint_counter = 3;

        let resp = PrefixResponse::from(&prefix);

        assert_eq!(resp.name, "kan");
        assert_eq!(resp.card_counter, 42);
        assert_eq!(resp.sprint_counter, 3);
    }

    #[test]
    fn test_prefix_response_round_trips_through_json() {
        let mut prefix = Prefix::new("feat");
        prefix.card_counter = 7;
        prefix.sprint_counter = 1;
        let resp = PrefixResponse::from(&prefix);

        let json = serde_json::to_string(&resp).unwrap();
        let decoded: PrefixResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, resp);
    }
}
