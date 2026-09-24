---
bump: minor
---

server: adds GET /v1/cards/lookup, resolving a card identifier (KAN-7 or a bare 7) against the locked Session::find_cards_by_identifier surface. Always answers 200 with an unpaginated JSON array of CardResponse; empty for no match or an unparseable identifier, never 404. backend-http: HttpBackend replaces its list_cards_by_prefix_and_number and list_cards_by_number declines with overrides against the new route, so kanban <url> card get KAN-N and the MCP card-by-identifier tools now work against a remote server.
