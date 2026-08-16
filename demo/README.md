# Kanban Demo

This directory contains the hero demo for the kanban TUI application. It showcases the core use case: creating a card, viewing the card list, and editing card metadata with an external editor.

## Quick Start

### Run the Demo Recording

```bash
nix develop .#demo --command bash demo/record.sh
```

This will:
1. Record a demo using VHS
2. Generate `demo.gif` in the `demo/` directory
3. Reset `demo/fixtures/demo.json` to its clean state

### View the Demo

Open `demo.gif` in any image viewer or browser.

## Understanding the Demo

### What the Demo Shows

1. **Start app** — Launch kanban with a demo fixture
2. **Select board** — Navigate and enter the "Kanban" board
3. **Create card** — Create a new card with `n`; it is selected automatically
4. **View details** — Press `Enter` to open card details view
5. **Edit metadata** — Navigate to description panel and edit with neovim
6. **Switch views** — Toggle the task list view with `V`
7. **Create a sprint** — Add a sprint for the unassigned cards
8. **Assign to sprint** — Multi-select cards, assign them, then filter by sprint
9. **Copy branch and quit** — Copy the card's git branch, quit, and paste it
10. **Drive it from the CLI** — `card get` and `card move` against the same file

### Key Bindings Used

- `j` — Navigate down (or next)
- `k` — Navigate up (or previous)
- `n` — Create new card
- `e` — Edit (opens external editor on description field)
- `v` — Toggle multi-select on the current card
- `V` — Switch task list view
- `a` — Assign card(s) to a sprint
- `T` — Open filter options
- `Space` — Toggle the highlighted entry (sprint picker, filter chips)
- `Shift+Y` — Copy the card's git branch to the clipboard
- `Enter` — Select/open, or confirm a dialog
- `q` — Quit

## Modifying the Demo

### The Demo Fixture

**File**: `demo/fixtures/demo.json`

This is a pre-crafted kanban board with:
- 1 board: "Kanban" (the kanban tool managing itself)
- 5 cards: Real development tasks (SQLite, sprint carry-over, CLI args, etc.)
- 3 columns: TODO, Doing, Complete
- 1 active sprint: "KAN-1/enhancements"

**Important**: This file is automatically reset after each recording by `record.sh`. Do not commit changes to it.

### The Demo Tape

**File**: `demo/demo.tape`

This VHS tape defines the exact sequence of keystrokes and timing. Edit this file to change:
- The demo narrative (what cards to create, edit, etc.)
- Timing (adjust `Sleep` values if interactions are too fast/slow)
- Visual settings (colors, dimensions, etc.)

#### VHS Syntax Notes

- `Type "text"` — Send text input
- `Enter` — Press the Enter key
- `Escape` — Press the Escape key
- `Type "j"` → `j` navigates down
- `Sleep 1000ms` — Wait 1 second
- Comments start with `#`

### Recording Tips

1. **After creating a card**, it is selected automatically — no `j` needed to reach it
2. **Sprint picker** — `j`/`k` move the cursor, `Space` checks the entry under the cursor, and `Enter` applies the checked entry. Without `Space` the `Enter` does nothing
3. **Timing matters** — Increase `Sleep` values if the app doesn't respond quickly
4. **Escape key** — Use `Escape` (not `Type "Escape"`) to press the key
5. **Test locally** — Run `kanban demo/fixtures/demo.json` manually to verify behavior before recording
6. **Recording needs a graphical session** — see the clipboard note under Troubleshooting

## Environment Setup

### Nix Dev Shell

The demo uses the flake's `demo` dev shell, which provides VHS and neovim:

```bash
nix develop .#demo --command bash demo/record.sh
```

The `record.sh` script handles:
- Directory navigation (cd into fixtures)
- Clean prompt via `PROMPT_COMMAND`
- VHS recording
- Output placement (`demo.gif` to `demo/`)
- Fixture reset

### nvim Editor Integration

The demo uses neovim for editing. The environment sets:

```bash
EDITOR=demo/nvim-editor.sh
```

This wrapper script ensures nvim starts with minimal config (`-u NONE --noplugin`) for reproducible recordings.

## Troubleshooting

### "nvim: no such file or directory"

The VHS process can't find nvim. Ensure you're running within the flake dev shell:

```bash
nix develop .#demo --command bash demo/record.sh
```

### Demo creates a board instead of a card

You forgot to press `Enter` after `j` to select and enter the board. The board selection screen requires both:
- `j` — Select the board
- `Enter` — Enter the board

### "failed to read clipboard" during recording

The copy-branch beat uses `Shift+Y`, which puts the branch on the system clipboard so VHS can `Paste` it into the shell. The TUI copies through arboard, which takes direct ownership of the X selection, so the selection disappears as soon as the app exits. Recording therefore needs a real graphical session: it fails in a headless TTY, and also under Xvfb with `autocutsel`. Run `nix develop .#demo --command bash demo/record.sh` from a desktop session.

### Card metadata doesn't save

The vim commands need proper timing:
- Wait 2500ms after pressing `e` for vim to open
- Press `i` to enter insert mode
- Type the new text
- Press `Escape` to exit insert mode (not `Type "Escape"`)
- Type `:wq` to save and quit

## File Structure

```
demo/
├── README.md              # This file
├── demo.tape              # VHS recording script
├── record.sh              # Run this to generate demo.gif
├── shell.nix              # Demo dev shell (requires pkgs from flake — use nix develop .#demo)
├── nvim-editor.sh         # Wrapper for nvim in demo
└── fixtures/
    └── demo.json          # Clean board fixture (auto-reset after recording)
```

## Next Steps

- **Modify the narrative**: Edit `demo.tape` to show different features
- **Test interactively**: Run `kanban demo/fixtures/demo.json` manually
- **Adjust timing**: If interactions feel rushed, increase `Sleep` values
- **Version the outputs**: Commit `demo.gif` when the demo is finalized

---

For more on VHS, see: https://github.com/charmbracelet/vhs
