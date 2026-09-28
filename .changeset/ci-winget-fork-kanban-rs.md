---
bump: patch
---

ci: the winget submission now opens its PR from the `kanban-rs/winget-pkgs` org fork instead of the personal `fulsomenko/winget-pkgs` fork, and the packaging README documents that `WINGET_TOKEN` must be a classic PAT with `public_repo` and `workflow` scopes. The package identifier changes from `fulsomenko.kanban` to `kanban-rs.kanban`; the first `kanban-rs.kanban` version is submitted manually (see the README) and `fulsomenko.kanban` is retired upstream once it lands.
