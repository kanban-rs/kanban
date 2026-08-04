#!/usr/bin/env bash
set -euo pipefail

BRANCH=$(git branch --show-current)

if [ "$BRANCH" = "master" ] || [ "$BRANCH" = "main" ]; then
  echo "Error: Cannot create changeset on master/main branch"
  exit 1
fi

# Determine bump type and description based on arguments
BUMP_TYPE="patch"
DESCRIPTION=""

if [ $# -gt 0 ]; then
  # Check if first argument is a valid bump type
  if [[ "$1" =~ ^(patch|minor|major)$ ]]; then
    BUMP_TYPE="$1"
    DESCRIPTION="${2:-}"
  else
    # First argument is treated as description, bump type defaults to patch
    DESCRIPTION="$1"
  fi
fi

if [ -z "$DESCRIPTION" ]; then
  BASE_BRANCH="${BASE_BRANCH:-develop}"
  # Find where this branch diverged from base, only show commits since then
  MERGE_BASE=$(git merge-base "$BASE_BRANCH" HEAD 2>/dev/null || echo "")
  if [ -n "$MERGE_BASE" ]; then
    COMMITS=$(git log --oneline "$MERGE_BASE"..HEAD --pretty=format:"- %s")
  else
    COMMITS=$(git log --oneline -1 --pretty=format:"- %s")
  fi

  if [ -z "$COMMITS" ]; then
    echo "Error: No commits found and no description provided"
    echo "Usage: $0 [patch|minor|major] \"Description of changes\""
    exit 1
  fi

  DESCRIPTION="$COMMITS"
  echo "Auto-generated description from commits:"
  echo "$DESCRIPTION"
  echo ""
fi

SANITIZED_BRANCH=$(echo "$BRANCH" | tr '/' '-' | tr '[:upper:]' '[:lower:]')

# Extract the issue ID (kan-NN) from ANYWHERE in the branch name, so a
# conventional-commit-style branch like "feat/kan-1046-configurable-bind-addr"
# still yields a card-first "kan-1046-..." changeset instead of being treated
# as cardless (which would bucket it under "Other Changes" in the changelog).
if [[ "$SANITIZED_BRANCH" =~ (kan-[0-9]+) ]]; then
  ISSUE_ID="${BASH_REMATCH[1]}"
  DESC="${SANITIZED_BRANCH##*"${ISSUE_ID}"}"   # everything after the issue ID
  DESC="${DESC#-}"                              # drop the leading separator
  if [ -n "$DESC" ]; then
    CHANGESET_FILE=".changeset/${ISSUE_ID}-${DESC}.md"
  else
    CHANGESET_FILE=".changeset/${ISSUE_ID}.md"
  fi
else
  CHANGESET_FILE=".changeset/${SANITIZED_BRANCH}.md"
fi

mkdir -p .changeset

cat > "$CHANGESET_FILE" <<EOF
---
bump: $BUMP_TYPE
---

$DESCRIPTION
EOF

echo "Created changeset: $CHANGESET_FILE"
echo "Bump type: $BUMP_TYPE"
echo "Description: $DESCRIPTION"
