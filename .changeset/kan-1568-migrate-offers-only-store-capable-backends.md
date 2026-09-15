---
bump: minor
---

`kanban migrate` no longer offers `http` as a target backend, since a remote
backend cannot be migrated to. `kanban-backend` adds `KanbanBackendFactory::is_remote`
(defaulted to false) and `KanbanBackendRegistry::local_names`; `kanban-backend-http`
declares itself remote; `kanban-service` adds `StoreManager::local_backend_names`;
`kanban-cli` builds the migrate command's possible values from it, so clap now
rejects `http` as an invalid value instead of failing deep inside the backend.
