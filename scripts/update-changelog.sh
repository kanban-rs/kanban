#!/usr/bin/env bash
set -euo pipefail

# Update CHANGELOG.md by prepending new entries from changesets
# Usage: update-changelog.sh <version> <changeset_files>
# Example: update-changelog.sh "0.2.0" ".changeset/kan-45-feature.md .changeset/kan-46-bugfix.md"

VERSION="${1:-}"
CHANGESETS="${2:-}"

if [ -z "$VERSION" ]; then
  echo "Error: VERSION required"
  echo "Usage: $0 <version> <changeset_files>"
  exit 1
fi

if [ -z "$CHANGESETS" ]; then
  echo "Warning: No changesets provided, skipping changelog update"
  exit 0
fi

CHANGELOG="CHANGELOG.md"

# Create backup
if [ -f "$CHANGELOG" ]; then
  cp "$CHANGELOG" "$CHANGELOG.bak"
fi

# Collect all changeset descriptions
ENTRIES=""
for changeset in $CHANGESETS; do
  # Extract description (everything after the second ---)
  description=$(sed -n '/^---$/,/^---$/!p' "$changeset" | sed '/^---$/d' | sed '/^$/d')
  if [ -n "$description" ]; then
    ENTRIES="$ENTRIES$description
"
  fi
done

# Create new changelog entry with version and date
DATE=$(date +%Y-%m-%d)
NEW_ENTRY="## [$VERSION] - $DATE

$ENTRIES
"

# Prepend to changelog (or create if doesn't exist)
if [ -f "$CHANGELOG" ]; then
  {
    echo "$NEW_ENTRY"
    cat "$CHANGELOG"
  } > "$CHANGELOG.tmp"
  mv "$CHANGELOG.tmp" "$CHANGELOG"
else
  echo "$NEW_ENTRY" > "$CHANGELOG"
fi

echo "Updated $CHANGELOG with version $VERSION"

# Commit the changelog update
git add "$CHANGELOG"
git commit -m "chore: update changelog for version $VERSION"
