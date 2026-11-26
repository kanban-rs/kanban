---
bump: patch
---

Create a card over a remote (HTTP) backend from the TUI

- `App::create_card` now routes the existing-column case over a remote backend through `KanbanContext::create_card_from_spec` (→ `RemoteWrites::create_card`) instead of the local-only `execute_with_extra` batch, which `require_local_only` rejects with `Unsupported` on a remote backend
- Auto-complete (target column has a `Done` default status) issues a follow-up `update_card` over the wire against the id the create returned, instead of being folded into the create's atomic batch
- The create-card sprint picker's selection travels with the create as `NewCard.sprint_id` over the wire
- Exposes `test_helpers::remote_backend` (in-memory store + injected `RemoteWrites`) from `kanban-service` so dependent crates' tests — including `kanban-tui`'s — can drive service/handler code against a remote-shaped backend without a real HTTP server, via a new `App::test_with_backend` constructor

Out of scope (tracked separately): creating a card when the target column does not exist yet, and creating into a column that already holds cards — both still take the local path and error with `Unsupported` over remote.
