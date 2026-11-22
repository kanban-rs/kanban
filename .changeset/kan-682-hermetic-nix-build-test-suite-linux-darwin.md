---
bump: patch
---

CI now exercises a fully hermetic Nix build and the entire test suite on both
Linux and Darwin for pull requests into master. The workspace test suite runs
inside the Nix sandbox via a new flake check rather than an impure development
shell, and the package builds on darwin with the modern apple-sdk pattern so the
clipboard integration links correctly. This underpins nixpkgs support on macOS.
