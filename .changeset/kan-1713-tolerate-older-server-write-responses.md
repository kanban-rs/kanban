---
bump: patch
---

api,backend-http: a write that an older kanban server commits is now reported as success with Invalidation::All when the server answers with a bare entity or 204 No Content, instead of a serialization error after the commit. Adds kanban_server::test_helpers::StubServer for old-server tests.
