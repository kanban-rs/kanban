#!/usr/bin/env bash
# Fails the build if crates/kanban-view/Cargo.toml declares a [dependencies]
# entry outside a fixed allowlist. kanban-view sits below kanban-tui/kanban-web
# and above kanban-domain/kanban-core; it must stay free of any TUI rendering
# framework (ratatui, crossterm) and of kanban-service, so later cards that
# move view-adjacent logic into it are provably decoupled at compile time, not
# just by convention.
#
# This supersedes an earlier version of this guard that grepped the resolved
# `cargo tree` output for the literal string "ratatui" — that only ever caught
# ratatui specifically, and the compiler already guarantees kanban-view can't
# *use* a crate it doesn't depend on. What the compiler does NOT catch is a
# future contributor *declaring* a new dependency (crossterm, tokio,
# kanban-service, ...) in Cargo.toml and then using it freely. An allowlist
# catches that at declaration time regardless of which crate it is, without
# needing a name added here for every framework that shouldn't leak in.
set -euo pipefail

CARGO_TOML="crates/kanban-view/Cargo.toml"

ALLOWED_DEPS=(
  kanban-core
  kanban-domain
  serde
  uuid
  chrono
)

if [ ! -f "$CARGO_TOML" ]; then
  echo "❌ $CARGO_TOML not found"
  exit 1
fi

# Extract dependency names from the [dependencies] table: lines of the form
# `name = ...` or `name.workspace = true`, up to the next `[section]` or EOF.
deps=$(awk '
  /^\[dependencies\]/ { in_deps = 1; next }
  /^\[/ { in_deps = 0 }
  in_deps && NF > 0 {
    line = $0
    sub(/^[[:space:]]+/, "", line)
    split(line, parts, /[[:space:]=.]/)
    if (parts[1] != "") print parts[1]
  }
' "$CARGO_TOML")

violations=0
while IFS= read -r dep; do
  [ -z "$dep" ] && continue
  found=0
  for allowed in "${ALLOWED_DEPS[@]}"; do
    if [ "$dep" = "$allowed" ]; then
      found=1
      break
    fi
  done
  if [ "$found" -eq 0 ]; then
    echo "❌ kanban-view declares disallowed dependency: $dep"
    violations=1
  fi
done <<< "$deps"

if [ "$violations" -ne 0 ]; then
  echo ""
  echo "kanban-view's dependency set is intentionally locked to: ${ALLOWED_DEPS[*]}"
  echo "If a new dependency is genuinely needed, add it to ALLOWED_DEPS in"
  echo "scripts/check-kanban-view-deps-allowlist.sh as a deliberate decision,"
  echo "not as a side effect of an unrelated change."
  exit 1
fi

echo "✅ kanban-view dependency allowlist guard clean"
