---
bump: patch
---

persistence-json,persistence-sqlite: whole-graph and cross-backend coverage for the V14-V18 JSON migration chain (schema 9-13 on SQLite). Every prior test in this chain fixtured a bare board plus cards with empty columns/sprints/edges; the new tests seed a non-trivial graph (two columns, one WIP-limited, several cards, a sprint with bound cards, an archived card, and one edge of each of the three kinds) and assert it survives the full chain intact, plus a cross-backend agreement test comparing the JSON and SQLite prefix rows and card-prefix stamps for the same scenario. No migration defects found; the new tests are regression pins.
