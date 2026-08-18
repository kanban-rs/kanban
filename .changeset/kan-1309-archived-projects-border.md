---
bump: patch
---

tui: the Projects panel now uses the same yellow border as the archived-tasks list while the archived-projects view is open and that panel is focused, so both archived surfaces signal themselves the same way. An unfocused archived Projects panel stays neutral, matching the archived-tasks panel. The tint is keyed on the stack-aware base mode, so a confirm dialog opened over the archived-projects view keeps the underlay tinted rather than flipping back to the normal border colour.
