# Complete Examples for nixpkgs Submission

This document shows complete, ready-to-use examples of what your nixpkgs submission files should look like.

## 1. Maintainer Entry (maintainers/maintainer-list.nix)

Location in file: Find alphabetically where "fulsomenko" goes (f section)

```nix
# ... other maintainers ...

fulsomenko = {
  email = "your-email@example.com";
  github = "fulsomenko";
  githubId = 12345678;  # Replace with your actual ID from: curl https://api.github.com/users/fulsomenko | jq '.id'
  name = "Max Emil Blomstervall";
};

# ... rest of maintainers ...
```

**Optional: Include GPG key for signing commits**

```nix
fulsomenko = {
  email = "your-email@example.com";
  github = "fulsomenko";
  githubId = 12345678;
  name = "Max Emil Blomstervall";
  keys = [{
    fingerprint = "XXXX XXXX XXXX XXXX XXXX  XXXX XXXX XXXX XXXX XXXX";
  }];
};
```

## 2. Package Definition (pkgs/applications/terminal-ui/kanban/default.nix)

```nix
{ lib
, rustPlatform
, fetchFromGitHub
, nix-update-script
}:

rustPlatform.buildRustPackage rec {
  pname = "kanban";
  version = "0.1.10";

  src = fetchFromGitHub {
    owner = "fulsomenko";
    repo = "kanban";
    rev = "v${version}";
    # Hash should be: sha256-<base64-encoded-hash>
    # Get actual hash by running:
    # nix-build -A kanban 2>&1 | grep "got:" | awk '{print $NF}'
    hash = "sha256-abcdef0123456789abcdef0123456789abcdef0123456=";
  };

  # Hash of Cargo dependencies
  # Get by running the build with a placeholder and using the error message
  cargoHash = "sha256-xyzabc9876543210xyzabc9876543210xyzabc9876543=";

  meta = {
    description = "A terminal-based project management solution";
    longDescription = ''
      Kanban is a terminal-based kanban/project management tool written in Rust,
      inspired by lazygit's interface design.

      Features:
      - Fast and responsive TUI with ratatui
      - Keyboard-driven navigation with vim-like shortcuts
      - JSON file persistence with import/export
      - Task management with priority levels and story points
      - Sprint planning and management
      - Multi-select operations for bulk actions
      - Automatic timestamp tracking for cards
    '';
    homepage = "https://github.com/fulsomenko/kanban";
    downloadPage = "https://github.com/fulsomenko/kanban/releases";
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ fulsomenko ];
    mainProgram = "kanban";
    platforms = lib.platforms.all;
  };

  passthru.updateScript = nix-update-script { };
}
```

## 3. all-packages.nix Entry

Location: In `pkgs/top-level/all-packages.nix`, find the terminal-ui section:

```nix
# In the terminal user interface section, add:

kanban = callPackage ../applications/terminal-ui/kanban { };

# Example context (alphabetical order):
# ...
# kak-lsp = callPackage ../applications/terminal-ui/kak-lsp { };
# kanban = callPackage ../applications/terminal-ui/kanban { };
# kitty = callPackage ../applications/terminal-ui/kitty { };
# ...
```

## 4. Complete Git Commit

```bash
# From nixpkgs root directory

git add maintainers/maintainer-list.nix
git add pkgs/applications/terminal-ui/kanban/default.nix
git add pkgs/top-level/all-packages.nix

git commit -m "Add kanban - terminal-based project management tool"

# Or with extended message:
git commit -m "Add kanban - terminal-based project management tool

- New package: kanban v0.1.10
- License: Apache-2.0
- Maintainer: fulsomenko
- Binary: kanban (terminal UI)

Kanban is a terminal-based kanban/project management tool written in
Rust with a focus on keyboard-driven navigation, file persistence, and
sprint management.

Features:
- TUI with ratatui and crossterm
- Vim-like keyboard shortcuts
- JSON import/export
- Task management with metadata
- Sprint planning and tracking
- Multi-select operations"
```

## 5. Hash Calculation Process (Step-by-step)

### Finding src hash:

```bash
cd nixpkgs

# Create default.nix with placeholder src hash
cat > pkgs/applications/terminal-ui/kanban/default.nix << 'EOF'
{ lib, rustPlatform, fetchFromGitHub, nix-update-script }:

rustPlatform.buildRustPackage rec {
  pname = "kanban";
  version = "0.1.10";

  src = fetchFromGitHub {
    owner = "fulsomenko";
    repo = "kanban";
    rev = "v${version}";
    hash = "sha256-0000000000000000000000000000000000000000000=";
  };

  # ... rest of definition ...
EOF

# Build and capture error
nix-build -A kanban 2>&1 | grep -A 1 "got:"

# Output will be something like:
# hash mismatch in fixed-output derivation '/nix/store/...'
#   expected: sha256-0000000000000000000000000000000000000000000=
#   got:      sha256-abc123def456abc123def456abc123def456abc1234=

# Use the "got:" value to update the hash in default.nix
```

### Finding cargoHash:

```bash
# Same process but for cargoHash - follow same steps as above
# The nix-build command will report the actual cargoHash needed
```

## 6. PR Title and Description

**Title:**
```
Add kanban - terminal-based project management tool
```

**Body:** (Use PR_DESCRIPTION_TEMPLATE.md provided in the repository)

## 7. Testing Before Submission

```bash
# Build test
nix-build -A kanban
echo "Build result: $?"

# Binary test
./result/bin/kanban --help

# Shell test (optional)
nix shell .#kanban
kanban

# Cleanup
rm -f result
```

## 8. Command Reference

### Getting your GitHub ID:
```bash
curl https://api.github.com/users/fulsomenko | jq '.id'
# Output: 12345678
```

### Finding where you go in maintainers list:
```bash
# Search for entries starting with 'f'
grep -n "^  f" maintainers/maintainer-list.nix | head -20
```

### Checking if package builds:
```bash
nix-build -A kanban --check
```

### Checking style:
```bash
# Nixpkgs has automated checks for style
# They run automatically on PRs (ofborg bot)
# But you can check locally:
nix fmt pkgs/applications/terminal-ui/kanban/default.nix
```

## 9. Expected Build Output

When everything is working correctly, you should see:

```
building path(s) '/nix/store/...-kanban-0.1.10'
Preparing sources...
unpacking https://github.com/fulsomenko/kanban/archive/v0.1.10.tar.gz...
source root is kanban-0.1.10
patching sources...
Compiling kanban...
   Compiling kanban-core v0.1.10 (...)
   Compiling kanban-domain v0.1.10 (...)
   Compiling kanban-tui v0.1.10 (...)
   Compiling kanban-cli v0.1.10 (...)
    Finished release [optimized] target(s) in XXs
installing...
strip is /usr/bin/strip
stripping (with command strip and flags -S -p) in  /nix/store/.../bin
/nix/store/XXXXX-kanban-0.1.10
```

## 10. Post-Merge Integration

After your PR is merged, update your main repository:

```markdown
# In your kanban README.md, add to installation section:

### Via nixpkgs

```bash
nix shell nixpkgs#kanban
```

If you're using NixOS:

```bash
# In your configuration.nix
environment.systemPackages = with pkgs; [ kanban ];
```

Or with flakes:

```nix
# In your flake.nix inputs
kanban.url = "github:fulsomenko/kanban";

# In outputs.packages
defaultPackage.x86_64-linux = nixpkgs.legacyPackages.x86_64-linux.kanban;
```
```

---

## File Locations Summary

```
nixpkgs/
├── maintainers/
│   └── maintainer-list.nix          # Add fulsomenko entry here
├── pkgs/
│   ├── applications/
│   │   └── terminal-ui/
│   │       └── kanban/
│   │           └── default.nix      # NEW: Copy from nixpkgs-default.nix
│   └── top-level/
│       └── all-packages.nix         # Add kanban line here
└── ...
```

## Validation Checklist

Before pushing to GitHub:

- [ ] Three files modified/created
- [ ] Maintainer entry is alphabetically ordered
- [ ] Package entry is alphabetically ordered
- [ ] Hash values are actual values, not placeholders
- [ ] `nix-build -A kanban` succeeds
- [ ] Binary runs: `./result/bin/kanban --help`
- [ ] No trailing whitespace
- [ ] Commit message is clear and concise

---

Ready to submit! 🚀
