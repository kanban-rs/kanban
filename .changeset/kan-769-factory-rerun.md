---
bump: minor
---

Introduces a versioned v1 API DTO surface and a unified entity-construction factory across all four core entities (boards, columns, cards, sprints).

New capabilities:

- A versioned wire DTO layer (`kanban_service::api::v1`) with create, update, replace, and response types for every entity, decoupled from the internal domain model. Update requests follow JSON Merge Patch semantics (a field that is absent means no change, null means clear, a value means set); replace requests are true full replacements.
- Client-supplied IDs on create. You may now provide an id when creating an entity (enabling idempotent creation); a collision returns a conflict (ALREADY_EXISTS, HTTP 409 at the wire) instead of silently overwriting existing data.
- Every entity is now built through a single construction path: one funnel for brand-new entities and one for loading existing ones, with identity and timestamps supplied by the caller rather than generated deep inside the domain. Creation is now deterministic.

Behaviour changes to be aware of:

- The CLI `--json` output for entities is now projected through the v1 response types. Wire field names are snake_case and internal bookkeeping fields (per-board counters, sprint name-pool indices) are no longer exposed. Scripts that parse the CLI JSON output should expect the snake_case, counter-free shape.
- The MCP server now exposes the shared v1 create fields rather than its own per-tool create schemas, so the create inputs are identical across the MCP and HTTP surfaces.
- These wire-shape changes ship as a `minor` bump on purpose: pre-1.0, the CLI `--json` output and the MCP request shapes are not yet covered by semantic versioning. Treat them as evolving until the v1 API stabilises.

Reliability and internals:

- Persistence (both the JSON and SQLite backends) now round-trips every entity through a dedicated record type, so a field can no longer be silently dropped when saving or loading. On-disk and database formats are unchanged, so no data migration is required.
- Loading a board or column with a blank name (possible in older data) no longer fails. The blank name is migrated to "Untitled" on load and persisted on the next save; creating a new board or column with a blank name is still rejected.
- A compile-time lock keeps the DTO, persistence, and domain representations in sync: the boundary types are free of defaults and rest-patterns and every conversion is exhaustive, enforced by a CI guard, so adding a field fails the build until every layer handles it.
