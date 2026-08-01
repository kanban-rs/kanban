# kanban-web

Server-rendered web-client frontend for `kanban-service`, built on
[Topcoat](https://crates.io/crates/topcoat).

## Status

Early and read-only. There is a single route (`GET /`) that renders every
board, its columns, and each column's card titles as plain, unstyled HTML —
no CSS, no interactivity, no other pages yet.

## Running

```bash
cargo run -p kanban-web
```

Binds to `127.0.0.1:3000` by default (override with the `HOST` / `PORT` env
vars — see Topcoat's `topcoat::start`). Then visit `http://localhost:3000/`.

## Backend selection

Set one of these before running:

- `KANBAN_FILE` — path to a local JSON board file (default: `./kanban.json`
  in the current directory; created lazily if missing, so an empty board
  list just means no file was found yet).
- `KANBAN_SERVER_URL` — base URL of a running `kanban-server` instance, read
  through `HttpBackend` instead of the local JSON file. Read-only: writes
  aren't implemented on `HttpBackend` yet.

```bash
KANBAN_FILE=/path/to/your/kanban.json cargo run -p kanban-web
# or
KANBAN_SERVER_URL=http://127.0.0.1:8080 cargo run -p kanban-web
```

## Nix

There is no Nix package for this binary yet. `nix build .#kanban-web` builds
an unrelated static landing page (`web/default.nix`) with the same output
name — don't confuse the two.
