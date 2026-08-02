use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Typed identity for a connected client. Every mutation issued over HTTP
/// carries this ID so the audit log and ChangeEventFrame can attribute changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientId(Uuid);

impl ClientId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(Uuid::nil())
    }
}

impl Default for ClientId {
    fn default() -> Self {
        Self::nil()
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ClientId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<ClientId> for Uuid {
    fn from(id: ClientId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_id_new_generates_non_nil_id() {
        let id = ClientId::new();
        assert_ne!(id, ClientId::nil());
    }

    #[test]
    fn test_client_id_nil_is_zero_uuid() {
        assert_eq!(Uuid::from(ClientId::nil()), Uuid::nil());
    }

    #[test]
    fn test_client_id_from_uuid_round_trips() {
        let uuid = Uuid::new_v4();
        let client_id = ClientId::from(uuid);
        assert_eq!(Uuid::from(client_id), uuid);
    }

    #[test]
    fn test_client_id_serialize_deserialize_round_trips() {
        let id = ClientId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: ClientId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_client_id_display_matches_inner_uuid() {
        let uuid = Uuid::nil();
        let id = ClientId::from(uuid);
        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn test_client_id_equality_and_hash_consistency() {
        use std::collections::HashSet;
        let id = ClientId::nil();
        let mut set = HashSet::new();
        set.insert(id);
        assert!(set.contains(&id));
    }

    #[test]
    fn test_client_id_default_is_nil() {
        assert_eq!(ClientId::default(), ClientId::nil());
    }
}
