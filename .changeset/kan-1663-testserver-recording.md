---
bump: patch
---

server: add `TestServer::start_recording`, a `test-helpers` constructor that returns a `RequestLog` of every `(method, path)` the router received in arrival order, including requests answered 404 by the router's fallback. Additive only; the existing five constructors are unchanged.
