#!/usr/bin/env bash
set -euo pipefail

DEMO="$(cd "$(dirname "$0")" && pwd)"
DEMO_NAME="demo"

# cd into fixtures so "kanban demo.json" works (not "kanban demo/fixtures/demo.json")
cd "$DEMO/fixtures"

PROMPT_COMMAND='PS1="> "' vhs $DEMO/$DEMO_NAME.tape

# Move demo.gif to demo directory
mv "$DEMO_NAME".gif "$DEMO/$DEMO_NAME.gif"

# Reset demo.json fixture to clean state
git checkout demo.json 2>/dev/null || true

echo "Done: $DEMO/$DEMO_NAME.gif"
echo "Reset demo.json to clean state"
