/// Wire name of the unauthenticated, client-supplied identity header.
/// Lowercase, so it is usable with `http::HeaderName::from_static`. Shared by
/// the server's extractor and HTTP clients so the two cannot drift.
pub const CLIENT_ID_HEADER: &str = "x-kanban-client-id";

#[cfg(test)]
mod tests {
    use super::CLIENT_ID_HEADER;

    #[test]
    fn test_client_id_header_is_the_lowercase_wire_name() {
        assert_eq!(CLIENT_ID_HEADER, "x-kanban-client-id");
    }
}
