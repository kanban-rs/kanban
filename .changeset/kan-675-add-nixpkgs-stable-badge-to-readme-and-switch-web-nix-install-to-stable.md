---
bump: patch
---

kanban is now packaged in nixpkgs 26.05 stable. The README gains a dedicated
"nixpkgs stable" version badge alongside the existing unstable one, so you can
see the latest version available on each channel at a glance. The project
website's install instructions now point at the stable channel, installing with
`nix run nixpkgs#kanban` instead of the unstable flake reference.
