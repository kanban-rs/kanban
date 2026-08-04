#!/usr/bin/env bash
# Fails the build if `ratatui` or `crossterm` appears anywhere in kanban-view's
# normal dependency tree. kanban-view sits below kanban-tui/kanban-web and above
# kanban-domain/kanban-core; it must stay free of any TUI rendering framework
# so later cards that move view-adjacent logic into it are provably decoupled
# from ratatui/crossterm at compile time, not just by convention.
set -euo pipefail

tree_output=$(cargo tree -p kanban-view -e normal)

if echo "$tree_output" | grep -qiE 'ratatui|crossterm'; then
  echo "❌ kanban-view dependency tree contains a TUI rendering framework:"
  echo "$tree_output" | grep -iE 'ratatui|crossterm'
  echo ""
  echo "kanban-view must not depend on ratatui or crossterm, directly or transitively."
  exit 1
fi

echo "✅ kanban-view no-ratatui/crossterm compile-lock guard clean"
