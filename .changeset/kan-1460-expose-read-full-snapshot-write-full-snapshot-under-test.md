---
bump: minor
---

service: expose `read_full_snapshot` and `write_full_snapshot` as public functions, re-exported from the crate root under the existing `test-helpers` feature, so integration-test crates outside `kanban-service` can seed and read a full workspace snapshot directly.
