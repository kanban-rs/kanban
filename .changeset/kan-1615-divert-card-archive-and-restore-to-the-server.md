---
bump: minor
---

service,backend-http,server: divert card archive and restore to the remote when a card is backed by a server. Card archive/restore now check remote_card_writes() before any local read, and return the server invalidation verbatim over HttpBackend. A backend that carries remote_writes but no card-write support now declines with a per-op message (archive_card/restore_card) instead of the blanket create/update/delete fence text. The archive/restore server routes now wrap their CardResponse in MutationResponse so the invalidation travels over the wire, matching the other card mutation routes. Breaking: RemoteCardWrites gains two required methods, so any external impl RemoteCardWrites for X {} stops compiling.
