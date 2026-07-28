---
bump: minor
---

`kanban-server` now exposes a live change stream: `GET /v1/events` is a
Server-Sent Events (SSE) endpoint that pushes a small notification to every
connected client whenever any client successfully creates, updates, or
deletes something on the server.

This lets multiple clients stay in sync without polling — open the stream
once and receive a notification the moment something changes elsewhere, with
periodic keep-alive pings so the connection survives idle periods and
proxies. This is server-side plumbing for real-time collaboration; client
support for consuming this stream lands in a future release.
