---
bump: patch
---

backend-http: add HTTP-backed behavioural and parity coverage for the four card batch mutations (`archive_cards`, `move_cards`, `assign_cards_to_sprint`, `update_cards`) against a real `kanban-server`, pinning that the server's `Invalidation` (scoped `Entities` for move, `All` for archive and for a no-op reassign) is carried through verbatim, and that the local-vs-server divergences (an unknown id moves the rest instead of failing the whole batch; a card already on the target sprint still counts as succeeded) hold end to end. Docs-only otherwise: the server and backend-http READMEs now list the four `/v1/cards/batch/*` routes among the write routes that return the mutation's `Invalidation`.
