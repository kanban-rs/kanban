#!/usr/bin/env bash
# Fails the build if `ratatui` appears anywhere in kanban-view's normal
# dependency tree. kanban-view sits below kanban-tui/kanban-web and above
# kanban-domain/kanban-core; it must stay free of any TUI rendering framework
# so later cards that move view-adjacent logic into it are provably decoupled
# from ratatui at compile time, not just by convention. See KAN-1044/KAN-1051.
set -euo pipefail

tree_output=$(cargo tree -p kanban-view -e normal)

if echo "$tree_output" | grep -qi ratatui; then
  echo "❌ kanban-view dependency tree contains ratatui:"
  echo "$tree_output" | grep -i ratatui
  echo ""
  echo "kanban-view must not depend on ratatui, directly or transitively."
  exit 1
fi

echo "✅ kanban-view no-ratatui compile-lock guard clean"
