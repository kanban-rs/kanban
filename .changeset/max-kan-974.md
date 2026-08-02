---
bump: patch
---

Actions invoked through the in-app help overlay (`?`) now actually run instead of silently doing nothing for four of them: copying a card's branch name (`y`) or git checkout command (`Y`) from the card or sprint detail views, carrying over a completed sprint's tasks (`M`), and exporting all boards (`x`). These all already worked as direct keypresses; only reaching them through the help menu was broken.
