# VHS Demo: Multi-Card Labels — three-state marks
#
# What this shows (tight beats, ~25s):
#   B1: Launch kanban, select the one board.
#   B2: Open KAN-1, create three labels on the Create row.
#   B3: Create three more cards (KAN-12, KAN-13, KAN-14).
#   B4: Apply "urgent" to KAN-12 only (sets the [-] partial state).
#   B5: Multi-select KAN-12, KAN-13, KAN-14.
#   B6: Open Labels from multi-select. Three marks visible:
#       [-] urgent   (partial: on 1 of 3)
#       [ ]  storage (none)
#       [ ]  bug     (none)
#       Create "critical" → fills all 3  → [check].
#       Fill  "urgent"    → partial 1/3 → all 3 → [check].
#   B7: Pause on the final state, close and quit.
#
# How to run (from the fixtures directory):
#   cd demo/fixtures && vhs ../labels.tape
#   mv labels.gif ../labels.gif
#
# NOTE: the "[-] 6 cards…" count-collapse (partial > PARTIAL_KEY_LIMIT=5)
# is not shown here to keep the demo under 30s. It is unit-tested in:
#   components::label_manager::tests::partial_mark_lists_few_keys_and_collapses_many_to_a_count

Output labels.gif
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set Padding 10
Set Framerate 16
Set TypingSpeed 75ms

# ── B1: Launch + select board ───────────────────────────────────────────────
Type "kanban demo.json"
Enter
Sleep 1200ms
# One board, "Kanban". j → Enter.
Type "j"
Sleep 500ms
Enter
Sleep 1200ms

# ── B2: KAN-1 detail, create three labels ──────────────────────────────────
# KAN-1 is the first card in the TODO column.
Type "l"
Sleep 900ms
# Registry is empty.  entries = [Note("(no labels)"), Create].
# Cursor on Create, already in INSERT mode (red border).
# After each Enter the cursor lands back on Create OR the last created label:
#   - first label: cursor stays on Create (INSERT); next name types directly.
#   - second label: cursor moves to the label row (NORMAL); j → Create.
Type "storage"
Sleep 400ms
Enter
Sleep 600ms
# Cursor is on Create (INSERT mode) — type "urgent" directly, no j needed.
Type "urgent"
Sleep 400ms
Enter
Sleep 600ms
# Cursor is on the "urgent" row (NORMAL) — j → Create, then type "bug".
Type "j"
Sleep 200ms
Type "bug"
Sleep 400ms
Enter
Sleep 600ms
# [check] storage   [check] urgent   [check] bug
Escape
Sleep 500ms
Escape
Sleep 700ms

# ── B3: Create three new cards ─────────────────────────────────────────────
# Cursor is on KAN-1 (top of column). `n` → type title → Enter.
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
Type "n"
Sleep 300ms
Type "Record the demo"
Sleep 300ms
Enter
Sleep 600ms
# KAN-14 is the most recently created — cursor is on it.

# ── B4: Apply "urgent" to KAN-12 only ──────────────────────────────────────
# Navigate from KAN-14 up to KAN-12.
Type "k"
Sleep 250ms
Type "k"
Sleep 250ms
# Cursor is now on KAN-12.
Enter
Sleep 800ms
# Open Labels for KAN-12.
Type "l"
Sleep 900ms
# KAN-12 has no tags yet.  All 3 registry labels are [ ] (none on KAN-12).
# entries: [Label(0)=storage, Label(1)=urgent, Label(2)=bug, Create]
# Cursor → storage (Label 0).  j → urgent (Label 1).
Type "j"
Sleep 200ms
# Enter: is_on_all("urgent") = 0/1 = false → ApplyToCard → "urgent" on KAN-12.
Enter
Sleep 800ms
# [check] urgent.  Close dialog and card detail.
Escape
Sleep 500ms
Escape
Sleep 500ms

# ── B5: Multi-select KAN-12, KAN-13, KAN-14 ───────────────────────────────
# Cursor is on KAN-12 (where we just came from).
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

# ── B6: Labels from multi-select — three marks + create + fill ─────────────
Type "m"
Sleep 1200ms
# Managed cards: KAN-12 (tags: urgent), KAN-13 (none), KAN-14 (none).
# Registry: storage, urgent, bug.
#
#   rows:    [urgent, storage, bug]   (registry order)
#   on_some: 1  (urgent on KAN-12; storage/bug on none)
#   entries: [urgent(-), Divider, storage(), bug(), Create]
#   Cursor:  urgent (Label 0, first selectable)
#
#   Visible in the Labels box:
#     [-] urgent   KAN-12
#     ───────────────────────────
#     [ ]  storage
#     [ ]  bug
#     [Type label name…]
#
# Navigate to Create: urgent → storage → bug → Create  (3 j; Divider skipped).
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
# Cursor on Create (NORMAL mode — j navigated here from a label row).
# Type "critical": first char enters INSERT mode; the rest fill the buffer.
Type "critical"
Sleep 400ms
Enter
Sleep 1000ms
# "critical" is created and applied to all 3 managed cards → [check].
#
#   rows:    [urgent(-), critical(check), storage(), bug()]
#   entries: [urgent(0), critical(1), Divider(2), storage(3), bug(4), Create(5)]
#
#   `keep` preserves the old Create index (4) → now lands on "bug".
#   Cursor: "bug" (index 4).
#
#   Navigate up to "urgent" to fill it:
#     bug → storage → [Divider] → critical → urgent
Type "k"
Sleep 200ms
Type "k"
Sleep 200ms
Type "k"
Sleep 200ms
# Cursor on "urgent" [-] (partial: on KAN-12 only, 1 of 3).
# Enter: is_on_all("urgent") = 1/3 → false → ApplyToCard → all 3 now carry it.
Enter
Sleep 1200ms
#
#   Final state:
#     [check] urgent        ← was [-] KAN-12, now filled up to all 3
#     [check] critical
#     ───────────────────────────
#     [ ]  storage          ← never applied to KAN-12/13/14
#     [ ]  bug
#
# Hold the final state so it registers clearly in the GIF.
Sleep 1200ms

# ── B7: Close and quit ─────────────────────────────────────────────────────
# Escape closes the Labels dialog → back to card list (multi-select preserved).
Escape
Sleep 400ms
Sleep 500ms
Type "q"
Sleep 400ms
Escape
Sleep 400ms

# End
Sleep 2000ms
