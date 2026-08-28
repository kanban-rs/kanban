---
bump: patch
---

tui: hide the Sprint section in the create-card dialog when the active board has no active or planned sprints, shrinking the dialog to just Title/Column instead of a fixed-height picker with nothing to pick. Focus and Esc-to-close now key off the last VISIBLE field rather than assuming Sprint always closes the dialog, so a board with no such sprints (and, when its column is also fixed, no editable Column either) still cancels correctly; the latter case falls through to the existing title-only popup. The section reappears once sprints load or a re-open finds a different, sprint-bearing board. The picker's own row count is unchanged when the section is shown.
