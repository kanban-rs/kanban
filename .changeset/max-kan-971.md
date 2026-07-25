---
bump: patch
---

Three confirmation dialogs — sprint-prefix collision, storage conflict resolution, and the external-file-change prompt — now show footer hints that match what the keys actually do. Previously all three borrowed a generic list-picker's hints (`j`/`k`/`Enter`), which did nothing in any of these dialogs, while their real keys went unlisted. This was most serious on the external-change dialog: `k` (keep local changes, discarding the external write) looked like harmless "navigate up" in the footer, so it was easy to press by accident and silently lose someone else's saved changes. The footer now shows the real keys with accurate, specific descriptions, and the in-app help overlay (`?`) invoked from within any of these three dialogs now performs the same action the direct keypress does, instead of a mismatched or no-op one.
