---
bump: patch
---

tui: the external config editor shows the default background highlight hex (#0000FF) when the colour is unset, and saving that field only ever persists valid #RRGGBB values (invalid input is rejected or dropped, never stored)
