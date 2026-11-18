# VHS Demo: Multi-Card Labels — applied-state marks
#
# What this shows (tight beats, ~20s):
#   B1: Launch kanban, arrange the board, and add one card ("Labels!").
#   B2: Open "Labels!", create three labels: storage, urgent, bug.
#   B3: Create three more cards (KAN-12, KAN-13, KAN-14).
#   B4: Apply one label to KAN-12 only  ->  it lands on one card (partial).
#   B5: Multi-select KAN-12, KAN-13, KAN-14.
#   B6: Open Labels from the multi-select. Each label's mark shows its
#       applied state across the managed cards:
#         [-] storage   on 1 of the 3      (partial: lists the card key)
#         [ ] urgent    on none of the 3
#         [ ] bug       on none of the 3
#   B7: Hold the state, close the dialog and board, quit.
#
# The full [check] mark (a label present on ALL managed cards) is the same
# mark promoted, and is covered by the unit tests in
# components::label_manager::tests (full_mark_is_a_single_check,
# partial_mark_lists_few_keys_and_collapses_many_to_a_count); it appears
# live as soon as a label is applied to every managed card.
#
# How to run (from the fixtures directory):
#   cd demo/fixtures && vhs ../labels.tape
#   mv labels.gif ../labels.gif

Output labels.gif
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set Padding 10
Set Framerate 16
Set TypingSpeed 75ms

# ── B1: Launch + arrange + one card ────────────────────────────────────────
Type "cargo run --bin kanban demo.json"
Enter
Sleep 1200ms
Enter
Sleep 1200ms
Type "c"
Sleep 100ms
Type "h"
Sleep 100ms
Type "c"
Sleep 100ms
Type "h"
Sleep 100ms
Type "c"
Sleep 100ms
Type "h"
Sleep 100ms
Type "n"
Sleep 400ms
Type "Labels!"
Sleep 400ms
Enter
Sleep 200ms

# ── B2: create three labels (registry) ─────────────────────────────────────
Enter    # open the "Labels!" card detail view
Type "l" # open Manage Labels
Sleep 900ms
# Registry is empty.  The cursor sits on the Create row (INSERT mode), so a
# name types directly.  After each Enter a short key nudges the cursor so the
# next name lands in a fresh Create row.
Type "storage"
Sleep 400ms
Enter
Sleep 500ms
Type "j"   # nudge the cursor back so the next name is a fresh create
Sleep 100ms
Type "urgent"
Sleep 400ms
Enter
Sleep 500ms
Type "j"
Sleep 100ms
Type "j"
Sleep 200ms
Type "bug"
Sleep 400ms
Enter
Sleep 500ms
Type "j"
Sleep 100ms
# Registry now: storage, urgent, bug.
Escape
Sleep 500ms
Escape
Sleep 700ms

# ── B3: create three more cards ────────────────────────────────────────────
# `n` on a card -> type a title -> Enter.  Repeats from the card list.
Type "n"
Sleep 300ms
Type "Design label state marks"
Sleep 300ms
Enter
Sleep 600ms
Type "n"
Sleep 300ms
Type "Show applied partial none"
Sleep 300ms
Enter
Sleep 600ms
# KAN-14 (most recently created) is now under the cursor.

# ── B4: apply one label to KAN-12 only ─────────────────────────────────────
Type "k"
Sleep 250ms
Type "k"
Sleep 250ms
# Cursor on KAN-12.  Open its detail, then Manage Labels.
Enter
Sleep 800ms
Type "l"
Sleep 900ms
# A registry label is under the cursor; Enter toggles it on for this card.
# Applying it to ONE of the cards yields a PARTIAL state.
Type "j"
Sleep 200ms
Enter
Sleep 500ms
# Applied to KAN-12 only.  Close the dialog and the detail view.
Escape
Escape
Escape
Sleep 2250ms

# ── B5: multi-select KAN-12, KAN-13, KAN-14 ────────────────────────────────
# `v` toggles multi-select on each card.
Type "v"
Sleep 350ms
Type "j"
Sleep 350ms
Type "v"
Sleep 350ms
Type "j"
Sleep 350ms
Type "v"
Sleep 350ms
# KAN-12 + KAN-13 + KAN-14 are in the multi-select set.

# ── B6: open Labels and read the applied-state marks ───────────────────────
Type "m"
Sleep 1200ms
# The three registry labels, each marked by its applied state:
#   [-] storage   on 1 of the 3   (partial: the card key is listed)
#   [ ] urgent    on none
#   [ ] bug       on none
# Hold this state so it registers clearly in the GIF.
Sleep 1500ms

# ── B7: close and quit ─────────────────────────────────────────────────────
Type "j"
Sleep 350ms
Enter
Sleep 350ms
Escape
Sleep 550ms
Type "m"
Sleep 1500ms

# ── B8: close and quit ─────────────────────────────────────────────────────
Escape
Sleep 400ms
Sleep 500ms
Type "q"
Sleep 400ms
Escape
Sleep 400ms

# End
Sleep 2000ms
