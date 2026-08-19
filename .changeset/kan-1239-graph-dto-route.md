---
bump: minor
---

server: a card's dependency edges are now readable over HTTP at GET /v1/cards/{id}/graph, returning the card's parents, children, blockers, blocked cards and related cards; a request for a card that does not exist returns 404 rather than an empty result.
