---
bump: patch
---

core,domain: undoing an import no longer strands an archived dependency edge it added. The inverse-capture the import merge emits (`RemoveSpawns`/`RemoveBlocks`/`RemoveRelates`) now carries the edge's state, so undo removes the edge in whatever state the forward insert left it instead of silently no-opping against a tombstone. `EdgeStore` gains `remove_archived_directed_edge`/`remove_archived_undirected_edge` as archived-only mirrors of the existing active-only removers, which are unchanged.
