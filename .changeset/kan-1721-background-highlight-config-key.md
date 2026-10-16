---
bump: minor
---

core: adds HexColor (#RRGGBB only, either case, displayed uppercase) and the background_highlight config key with AppConfig::background_highlight_color(). Breaking (library API): AppConfig gains a public field; external exhaustive struct literals must add it.
