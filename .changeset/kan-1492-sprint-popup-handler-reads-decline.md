---
bump: patch
---

tui: sprint and sprint-popup handlers (`handle_activate_sprint_key`, `handle_complete_sprint_key`, `handle_carry_over_for_sprint`, `create_sprint`, `handle_assign_card_to_sprint_popup`, `handle_assign_multiple_cards_to_sprint_popup`, `handle_carry_over_sprint_popup`) now read sprints through the state-preserving `Model` accessors instead of the collapsing `Model::sprints()`, so a `NotLoaded` sprint tier surfaces an error banner instead of silently behaving as an empty collection.
