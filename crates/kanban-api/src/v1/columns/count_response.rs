use serde::{Deserialize, Serialize};

/// Response body for `GET /v1/columns/{column_id}/cards/count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardCountResponse {
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_count_response_serde_round_trip() {
        let resp = CardCountResponse { count: 7 };
        let json = serde_json::to_value(resp).unwrap();
        assert_eq!(json, serde_json::json!({"count": 7}));
        let back: CardCountResponse = serde_json::from_value(json).unwrap();
        assert_eq!(back, resp);
    }
}
