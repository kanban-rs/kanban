# Nixpkgs Submission - Quick Reference Card

Print this or keep it open while working through the submission process.

## 7-Phase Submission Overview

```
Phase 1: Fork & Branch (5 min)
  └─ Fork NixOS/nixpkgs → create add-kanban-package branch

Phase 2: Maintainer Registration (5 min)
  └─ Edit maintainers/maintainer-list.nix → add fulsomenko entry

Phase 3: Package Directory (15 min)
  └─ mkdir pkgs/applications/terminal-ui/kanban/
  └─ Copy nixpkgs-default.nix → default.nix

Phase 4: Calculate Hashes (10 min)
  └─ Get src hash → nix-build -A kanban (use error message)
  └─ Get cargoHash → same process

Phase 5: Register Package (5 min)
  └─ Edit pkgs/top-level/all-packages.nix → add kanban entry

Phase 6: Local Test (10 min)
  └─ nix-build -A kanban → ./result/bin/kanban --help

Phase 7: Submit (10 min)
  └─ git commit/push → Create PR on GitHub
```

## Key Files

| File | Location | Action |
|------|----------|--------|
| Maintainer Entry | `maintainers/maintainer-list.nix` | Add entry (alphabetical) |
| Package Definition | `pkgs/applications/terminal-ui/kanban/default.nix` | Create file |
| Package Registry | `pkgs/top-level/all-packages.nix` | Add one line |

## Commands

### Get GitHub ID
```bash
curl https://api.github.com/users/fulsomenko | jq '.id'
```

### Calculate Hashes
```bash
cd /path/to/nixpkgs
nix-build -A kanban 2>&1 | grep "got:"
# Use the output hash
```

### Test Build
```bash
nix-build -A kanban
./result/bin/kanban --help
```

### Commit & Push
```bash
git add maintainers/maintainer-list.nix
git add pkgs/applications/terminal-ui/kanban/default.nix
git add pkgs/top-level/all-packages.nix
git commit -m "Add kanban - terminal-based project management tool"
git push origin add-kanban-package
```

## Template Snippets

### Maintainer Entry
```nix
fulsomenko = {
  email = "your-email@example.com";
  github = "fulsomenko";
  githubId = YOUR_ID;
  name = "Max Emil Blomstervall";
};
```

### Package Entry
```nix
kanban = callPackage ../applications/terminal-ui/kanban { };
```

## Checklist

Before submitting:

- [ ] Forked nixpkgs
- [ ] Created feature branch
- [ ] Found GitHub ID
- [ ] Added maintainer entry
- [ ] Created kanban directory
- [ ] Copied default.nix template
- [ ] Calculated src hash
- [ ] Calculated cargoHash
- [ ] Updated all-packages.nix
- [ ] Built successfully
- [ ] Binary runs
- [ ] Committed with message
- [ ] Pushed to fork
- [ ] Created PR

## Help Resources

| Problem | Solution |
|---------|----------|
| Can't find GitHub ID | `curl https://api.github.com/users/fulsomenko \| jq '.id'` |
| Hash mismatch | Recalculate: `nix-build -A kanban 2>&1 \| grep "got:"` |
| Build fails | Check hashes first, then review error message |
| Package not found | Check capitalization in all-packages.nix |
| ofborg fails | Wait for full check (5-10 min), then review logs |

## PR Details

**Title:**
```
Add kanban - terminal-based project management tool
```

**Body:** Use PR_DESCRIPTION_TEMPLATE.md

## After Merge

Users can install with:
```bash
nix shell nixpkgs#kanban
```

Update README to mention this!

---

## Emergency Help

1. Check NIXPKGS_SUBMISSION.md (detailed steps)
2. Review NIXPKGS_EXAMPLES.md (complete examples)
3. Search nixpkgs for similar Rust packages
4. Ask #nixos on Libera.Chat IRC

## Expected Build Output

```
building path(s) '/nix/store/...-kanban-0.1.10'
...
Compiling kanban-cli v0.1.10
    Finished release [optimized] target(s) in XXs
installing...
/nix/store/XXXXX-kanban-0.1.10
```

## Timeline

| Phase | Duration | Total |
|-------|----------|-------|
| 1-2 | 10 min | 10 min |
| 3-4 | 25 min | 35 min |
| 5-6 | 15 min | 50 min |
| 7 | 10 min | 60 min |

**Total: ~1 hour (mostly waiting for builds)**

---

**Ready?** → Open NIXPKGS_CHECKLIST.md to begin! 🚀
