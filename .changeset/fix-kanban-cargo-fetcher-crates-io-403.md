---
bump: patch
---

Fix Nix packaging: switch cargo vendoring from importCargoLock to fetchCargoVendor to avoid crates.io 403 blocks on the curl User-Agent
