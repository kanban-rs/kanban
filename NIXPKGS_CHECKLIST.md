# Kanban → nixpkgs Submission Checklist

This checklist summarizes everything needed to get kanban into the official nixpkgs repository.

## Your Current Status ✅

Your project is **exceptionally well-prepared** for nixpkgs submission. All core requirements are already met:

### Pre-requisites (✅ ALL COMPLETE)

- [x] **Cargo.toml** with complete metadata
  - Version: 0.1.10
  - License: Apache-2.0 (compatible with nixpkgs)
  - Repository: https://github.com/fulsomenko/kanban
  - Description and homepage present

- [x] **Cargo.lock** (committed to repository)
  - Ensures reproducible builds
  - Required for nixpkgs Rust packages

- [x] **LICENSE.md** (Apache-2.0)
  - Proper license file included

- [x] **flake.nix** (exists and functional)
  - Development environment properly configured
  - Uses standard Nix patterns

- [x] **README.md** (comprehensive)
  - Clear feature list
  - Installation instructions
  - Architecture documentation
  - Usage examples

- [x] **Clean Git History**
  - Organized crate structure
  - Meaningful commits
  - Active development

## Next Steps (ACTION ITEMS)

### Phase 1: Prepare Your Fork ⏱️ ~15 minutes

1. **Fork nixpkgs**
   ```bash
   # Go to https://github.com/NixOS/nixpkgs and click "Fork"
   # Clone your fork
   git clone https://github.com/YOUR_USERNAME/nixpkgs.git
   cd nixpkgs
   git checkout master
   ```

2. **Create a feature branch**
   ```bash
   git checkout -b add-kanban-package
   ```

### Phase 2: Register as Maintainer ⏱️ ~5 minutes

1. **Find your GitHub ID**
   ```bash
   curl https://api.github.com/users/fulsomenko | jq '.id'
   # Note the numeric ID
   ```

2. **Edit `maintainers/maintainer-list.nix`**
   - Use MAINTAINER_TEMPLATE.nix as reference
   - Add your entry in alphabetical order
   - Include email, github username, githubId, and name

3. **Example:**
   ```nix
   fulsomenko = {
     email = "your-email@example.com";
     github = "fulsomenko";
     githubId = 12345678;
     name = "Max Emil Blomstervall";
   };
   ```

### Phase 3: Add Package Definition ⏱️ ~20 minutes

1. **Create directory**
   ```bash
   mkdir -p pkgs/applications/terminal-ui/kanban
   ```

2. **Create `pkgs/applications/terminal-ui/kanban/default.nix`**
   - Use `nixpkgs-default.nix` as template
   - Replace placeholder hashes with actual values (see below)

3. **Calculate hashes**
   ```bash
   # For src hash - Start with placeholder
   # src = fetchFromGitHub {
   #   ...
   #   hash = "sha256-0000000000000000000000000000000000000000000=";
   # };

   # Build to get actual hash
   nix-build -A kanban 2>&1 | grep "got:"
   # Use the output to replace the hash

   # Repeat for cargoHash
   ```

### Phase 4: Register Package ⏱️ ~5 minutes

1. **Edit `pkgs/top-level/all-packages.nix`**
   - Find the `terminal-ui` section (or create one)
   - Add: `kanban = callPackage ../applications/terminal-ui/kanban { };`
   - Keep alphabetical order

### Phase 5: Test Locally ⏱️ ~10 minutes

```bash
# From your nixpkgs directory
nix-build -A kanban

# Verify binary
./result/bin/kanban --help

# Should display help without errors
```

### Phase 6: Commit & Push ⏱️ ~5 minutes

```bash
git add maintainers/maintainer-list.nix
git add pkgs/applications/terminal-ui/kanban/default.nix
git add pkgs/top-level/all-packages.nix
git commit -m "Add kanban - terminal-based project management tool"
git push origin add-kanban-package
```

### Phase 7: Create Pull Request ⏱️ ~10 minutes

1. **Go to NixOS/nixpkgs**
2. **Click "New Pull Request"**
3. **Set base: NixOS/nixpkgs (master) ← your-fork (add-kanban-package)**
4. **Use PR_DESCRIPTION_TEMPLATE.md as the PR body**
5. **Submit!**

## Total Time Estimate

- Preparation: **70 minutes** (but mostly waiting for builds)
- Actual work: **35 minutes**

## Files Provided in This Repository

To help with your submission, the following files have been created:

1. **NIXPKGS_SUBMISSION.md** - Comprehensive step-by-step guide
2. **nixpkgs-default.nix** - Template for `pkgs/applications/terminal-ui/kanban/default.nix`
3. **MAINTAINER_TEMPLATE.nix** - Template for maintainer registration
4. **PR_DESCRIPTION_TEMPLATE.md** - Ready-to-use PR description
5. **NIXPKGS_CHECKLIST.md** - This file

## Common Pitfalls to Avoid

1. ❌ **Hash mismatches** → Recalculate both `src` and `cargoHash`
2. ❌ **Missing maintainer entry** → Add yourself to `maintainers/maintainer-list.nix` first
3. ❌ **Wrong directory structure** → Use `pkgs/applications/terminal-ui/kanban/`
4. ❌ **Forgetting Cargo.lock** → It's already in your repo ✅
5. ❌ **Platform-specific issues** → Your deps are all cross-platform ✅

## Success Indicators

After your PR is created:

- ✅ ofborg will run automated checks
- ✅ You'll see build results for multiple platforms
- ✅ Reviewers will provide feedback (usually friendly!)
- ✅ Once approved, your package goes into nixpkgs
- ✅ Users can then do: `nix shell nixpkgs#kanban`

## After Merge

Once your PR is merged:

1. **Update README.md** - Add nixpkgs installation method:
   ```markdown
   ### Via nixpkgs
   ```bash
   nix shell nixpkgs#kanban
   ```
   ```

2. **Monitor maintenance** - The `nix-update-script` in the package definition will help with future updates

3. **Help other users** - You'll now be the maintainer for this package in nixpkgs

## Support Resources

- **Nixpkgs Manual**: https://nixos.org/manual/nixpkgs/unstable/
- **Contributing Guide**: https://github.com/NixOS/nixpkgs/blob/master/CONTRIBUTING.md
- **Rust in Nixpkgs**: https://github.com/NixOS/nixpkgs/blob/master/doc/language-support/rust.md
- **ofborg Bot**: Automatic testing bot that runs on all PRs
- **IRC Chat**: #nixos on Libera.Chat for live help

## Final Checklist Before Submitting PR

- [ ] forked NixOS/nixpkgs
- [ ] Created feature branch from `master`
- [ ] Added maintainer entry (alphabetically ordered)
- [ ] Created package directory: `pkgs/applications/terminal-ui/kanban/`
- [ ] Created `default.nix` with correct hashes
- [ ] Added entry to `pkgs/top-level/all-packages.nix`
- [ ] Tested build: `nix-build -A kanban`
- [ ] Tested binary: `./result/bin/kanban --help`
- [ ] Committed with clear message
- [ ] Pushed to your fork
- [ ] Created PR with descriptive title and body
- [ ] Filled PR body with template content

---

## Questions?

If you have questions during the process:

1. Check **NIXPKGS_SUBMISSION.md** for detailed explanations
2. Review existing Rust packages in nixpkgs for reference
3. Ask in the #nixos IRC channel on Libera.Chat
4. Check nixos.org/manual for official documentation

**Good luck! Your package is ready. 🚀**
