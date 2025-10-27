# Submitting Kanban to nixpkgs

This guide walks through submitting the `kanban` terminal application to the official nixpkgs repository.

## Prerequisites

- GitHub account with SSH keys configured for git operations
- Rust toolchain (for local verification)
- Basic familiarity with git and GitHub pull requests
- A fork of [NixOS/nixpkgs](https://github.com/NixOS/nixpkgs)

## Step 1: Register as a Maintainer

Before submitting the package, you need to register as a maintainer in nixpkgs.

### 1a. Find Your Maintainer Entry

In your forked nixpkgs repository, locate the file:
```
maintainers/maintainer-list.nix
```

### 1b. Add Your Entry

Add your maintainer information in alphabetical order. Find where your GitHub username fits alphabetically and add an entry following the existing format:

```nix
fulsomenko = {
  email = "your-email@example.com";
  github = "fulsomenko";
  githubId = YOUR_GITHUB_USER_ID;  # You can find this at https://api.github.com/users/fulsomenko
  name = "Max Emil Blomstervall";
  keys = [{
    fingerprint = "YOUR_GPG_FINGERPRINT";  # Optional but recommended
  }];
};
```

**Finding Your GitHub ID:**
```bash
curl https://api.github.com/users/fulsomenko | jq '.id'
```

## Step 2: Create the Package Definition

### 2a. Create Directory Structure

In your forked nixpkgs, create the package directory:

```bash
mkdir -p pkgs/applications/terminal-ui/kanban
```

### 2b. Create default.nix

Create `pkgs/applications/terminal-ui/kanban/default.nix`:

```nix
{ lib
, rustPlatform
, nix-update-script
}:

rustPlatform.buildRustPackage rec {
  pname = "kanban";
  version = "0.1.10";

  src = fetchFromGitHub {
    owner = "fulsomenko";
    repo = "kanban";
    rev = "v${version}";
    hash = "sha256-XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX=";
  };

  cargoHash = "sha256-YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY=";

  meta = {
    description = "A terminal-based project management solution";
    longDescription = ''
      A terminal-based kanban/project management tool inspired by lazygit,
      built with Rust. Features include file persistence, keyboard-driven
      navigation, multi-select capabilities, and sprint management.
    '';
    homepage = "https://github.com/fulsomenko/kanban";
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ fulsomenko ];
    mainProgram = "kanban";
    platforms = lib.platforms.all;
  };

  passthru.updateScript = nix-update-script { };
}
```

### 2c. Calculate Hashes

You need to calculate the correct hashes for your package.

**For src hash (SHA256 of the release tarball):**

1. Create a temporary file with placeholder hash:
```nix
src = fetchFromGitHub {
  owner = "fulsomenko";
  repo = "kanban";
  rev = "v0.1.10";
  hash = "sha256-0000000000000000000000000000000000000000000=";
};
```

2. Run nix-build to get the actual hash:
```bash
cd /path/to/nixpkgs
nix-build -A kanban 2>&1 | grep "got:" | awk '{print $NF}' | tr -d "'"
```

3. Update the hash with the correct value.

**For cargoHash (locked dependencies):**

Same process - use placeholder, build, and replace with actual value.

Alternatively, you can use `nix hash` command:
```bash
nix hash file file.tar.gz --type sha256 --base64
```

## Step 3: Register Package in all-packages.nix

### 3a. Find the Right Location

Open `pkgs/top-level/all-packages.nix` and locate the `terminal-ui` section (or create one if it doesn't exist).

### 3b. Add Package Entry

Add your package in alphabetical order:

```nix
kanban = callPackage ../applications/terminal-ui/kanban { };
```

## Step 4: Test the Package

### 4a. Local Build Test

```bash
cd /path/to/nixpkgs
nix-build -A kanban
```

Expected output:
```
/nix/store/XXXXXXX-kanban-0.1.10
```

### 4b. Verify the Binary

```bash
./result/bin/kanban --help
```

Should display the help output without errors.

### 4c. Test with nix shell (optional)

```bash
nix shell .#kanban
kanban  # Should launch the TUI
```

## Step 5: Create the Pull Request

### 5a. Commit Your Changes

```bash
git add maintainers/maintainer-list.nix
git add pkgs/applications/terminal-ui/kanban/default.nix
git add pkgs/top-level/all-packages.nix
git commit -m "Add kanban - terminal-based project management tool"
```

### 5b. Push to Your Fork

```bash
git push origin add-kanban-package
```

### 5c. Create Pull Request

Navigate to NixOS/nixpkgs and create a PR with this description:

```markdown
## Description

Add the `kanban` package - a terminal-based project management/kanban tool.

**Package Details:**
- **Name:** kanban
- **Version:** 0.1.10
- **License:** Apache-2.0
- **Maintainer:** @fulsomenko
- **Repository:** https://github.com/fulsomenko/kanban

**Features:**
- Terminal UI built with ratatui
- Keyboard-driven navigation (vim-like)
- File persistence with JSON import/export
- Task/card management with metadata
- Sprint management and planning
- Multi-select and bulk operations

**Testing:**
- [x] `nix-build -A kanban` succeeds
- [x] Binary executes without errors: `./result/bin/kanban --help`
- [x] Maintainer registered in maintainers/maintainer-list.nix

**Related Issues:**
Closes #(issue number if applicable)
```

## Common Issues and Solutions

### Issue: "attribute 'fetchFromGitHub' missing"

**Solution:** Add `fetchFromGitHub` to your package function arguments:

```nix
{ lib
, rustPlatform
, fetchFromGitHub
}:
```

### Issue: Cargo hash mismatch

**Solution:** The cargoHash changes when dependencies change. Delete it temporarily and run the build to get the actual value:

```bash
# Remove cargoHash = "..."; line temporarily
nix-build -A kanban 2>&1 | grep "got:"
# Update with the provided hash
```

### Issue: "permission denied" when running binary

**Solution:** Make sure the binary is in the result:

```bash
ls -la ./result/bin/kanban
# Should show executable permission (x)
```

## PR Review Process

1. Automated checks will run (ofborg):
   - Build checks on multiple platforms
   - Nix formatting checks
   - License compliance

2. Human reviewers will:
   - Check code quality
   - Verify metadata accuracy
   - Ensure no breaking changes

3. Common feedback items:
   - Missing `meta` fields
   - Outdated documentation links
   - Platform-specific build failures

## After Merge

Once merged to nixpkgs:

1. Users can install with:
   ```bash
   nix shell nixpkgs#kanban
   # or
   nix-env -i kanban  # For NixOS
   ```

2. Update your repository's README to mention nixpkgs availability:
   ```markdown
   ## Installation

   ### Via nixpkgs
   ```bash
   nix shell nixpkgs#kanban
   ```
   ```

3. The package will be available in the next nixpkgs release.

## References

- [Nixpkgs Manual - Contributing](https://nixos.org/manual/nixpkgs/unstable/#contributing)
- [Nixpkgs Manual - Rust](https://nixos.org/manual/nixpkgs/unstable/#sec-language-rust)
- [Rust in Nixpkgs - Best Practices](https://github.com/NixOS/nixpkgs/tree/master/doc/language-support/rust)
- [Official nixpkgs Repository](https://github.com/NixOS/nixpkgs)

## Troubleshooting

For additional help:

1. **Search existing issues** on NixOS/nixpkgs GitHub
2. **Check the wiki** at https://nixos.wiki/
3. **Ask in chat** - IRC channel: #nixos on Libera.Chat or community forums
4. **Review similar packages** in nixpkgs for reference implementations

Good luck with your submission! 🚀
