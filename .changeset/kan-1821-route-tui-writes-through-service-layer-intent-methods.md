<!--
type: fix
crate: tui
-->

Fix the class of TUI write operations that fail on remote backends with
`operation not supported by this backend`, part one of KAN-1821. This change
fixes two concrete, user-visible bugs:

- **Creating a board over a remote backend** (e.g. over HTTP) is now possible:
  `create_board` in the TUI now detects a remote backend up front and diverts
  to a remote-safe, per-entity creation path (the same `NewBoard`/`NewColumn`
  spec-based creation the card column-creation flow already used), instead of
  falling through to the local-only command-log batch, which is rejected for
  remote backends. The existing, tested local single-undo-transaction behavior
  is fully preserved.
- **The TUI's "archive on animation tick" flow no longer silently skips
  repacking column positions**: the local branch of the service-layer
  `archive_cards` now actually runs the `CompactColumnPositions` housekeeping
  step (as part of the same single, undoable batch, matching the original TUI
  behavior), and it correctly does **not** touch columns that hold no
  *live* cards being archived (a latent bug where archiving an already-
  archived card in the same batch would needlessly re-pack an unaffected
  column's live cards).

Two new, properly-specified, local/remote-safe service-layer methods are added
for future TUI handler coverage (part of the broader KAN-1821 unification):

- `KanbanOperations::delete_cards` — batch card deletion: one undo entry
  locally, a (documented) per-card wire-call sequence remotely.
- `KanbanOperations::swap_column_positions` — swap two columns' positions,
  with the two positions supplied by the caller (the only side that
  authoritatively knows them for a remote backend).

These two, plus the pre-existing `archive_cards`, are covered by new local/
remote parity tests (JSON + SQLite + recording-mock remote) in
`kanban-tui/tests/intent_methods_local_remote_parity.rs`, exercising local
undo semantics and remote wire-call ordering for each.
