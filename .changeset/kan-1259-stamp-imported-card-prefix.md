---
bump: patch
---

Imported cards written before the prefix was stored now have it filled in during import, resolved the same way the storage migration resolves it. Such cards previously arrived with no prefix, which left them unreachable by their own identifier and let the restored counter land on a namespace nothing addressed them by. The value written does not depend on the importing workspace's configuration, so a given file imports identically everywhere and matches what opening it directly would produce. One consequence is visible: a workspace that had set `default_card_prefix` saw these prefix-less cards displayed under that configured value in branch names and card detail, and after import they carry the built-in `task` prefix instead, because that is what the file itself implies and what opening it directly has always produced.
