---
bump: minor
---

Archived cards and boards are now fully functional everywhere. An archived
entity is just its ordinary card or board plus a marker, so you can view,
browse, edit, and restore archived items exactly like live ones. The
live/archived distinction is now a filter at each surface rather than a
separate, limited mode.

- CLI: `card list` and `board list` gain `--archived` (archived only) and
  `--include-archived` (live and archived together), plus `board archive`,
  `board restore`, and `board delete-archived` commands.
- MCP: `list_cards` and `list_boards` take an `archived` filter (`exclude`,
  `only`, `include`), and there are board archive, restore, and delete-archived
  tools. The separate `list_archived_cards` tool has been removed; use
  `list_cards` with `archived: "only"` instead.
- TUI: an archived boards view you can open into and browse like any live board,
  drilling into its columns and cards, with archive, restore, and
  permanent-delete affordances.

Under the hood the archival model was unified onto a single reference marker
(the entity stays live and a marker records that it is archived), which removes
the old separate archived collections and their drift. JSON files upgrade
automatically from format V9 to V10 on first open, lifting embedded archived
entities into references and writing a `.v9.backup` first; SQLite needs no
migration. The change is backward compatible.
