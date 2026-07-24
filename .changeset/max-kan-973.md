---
bump: patch
---

The Sprint Detail screen's footer hints now match what actually happens when you press the keys. Previously `s`, `y`, and `Y` were shown as available (assign to sprint, copy branch name, copy git checkout command) but did nothing when pressed; they now work, reusing the same assign-to-sprint picker and clipboard-copy behavior already available elsewhere in the app. Conversely, `d` (archive the selected task or tasks) was fully functional but never shown in the footer, so it could be pressed by accident with no indication of what it does; it's now advertised alongside the other keys.
