---
bump: patch
---

docs: the README's nixpkgs stable and unstable badges read the packaged version straight from `pkgs/by-name/ka/kanban/package.nix` on the `nixos-26.05` and `nixos-unstable` branches through a shields.io regex badge, instead of from Repology, whose domain is suspended and no longer resolves, leaving both badges as unstyled alt text.
