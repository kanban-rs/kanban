use kanban_domain::Prefix;
use serde::{Deserialize, Serialize};

/// Response body for prefix reads. Mirrors the domain row: `name` is the
/// namespace's identity (already normalised). `last_card_number` and
/// `last_sprint_number` are HIGH-WATER MARKS (the last number handed out,
/// not a count) -- named to say so on the wire, since the domain fields they
/// come from (`Prefix::card_counter`/`sprint_counter`) read as counts and are
/// not. Read-only -- there is no write route, so `Deserialize` is derived
/// only for round-trip tests / client use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefixResponse {
    pub name: String,
    pub last_card_number: u32,
    pub last_sprint_number: u32,
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
            last_card_number: *card_counter,
            last_sprint_number: *sprint_counter,
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
        assert_eq!(resp.last_card_number, 42);
        assert_eq!(resp.last_sprint_number, 3);
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

    #[test]
    fn test_prefix_response_wire_fields_are_high_water_mark_named() {
        let prefix = Prefix::new("kan");
        let resp = PrefixResponse::from(&prefix);

        let json = serde_json::to_string(&resp).unwrap();

        assert!(json.contains("\"last_card_number\""), "json: {json}");
        assert!(json.contains("\"last_sprint_number\""), "json: {json}");
        assert!(!json.contains("card_counter"), "json: {json}");
        assert!(!json.contains("sprint_counter"), "json: {json}");
    }
}
