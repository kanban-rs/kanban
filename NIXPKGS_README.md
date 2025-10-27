# Nixpkgs Submission Materials

This directory contains all the materials needed to submit the `kanban` package to the official nixpkgs repository.

## Quick Start

Your project is **ready to submit**! Follow this order:

1. **Start here**: Read `NIXPKGS_CHECKLIST.md` (5 min)
2. **Detailed guide**: Follow `NIXPKGS_SUBMISSION.md` (reference as needed)
3. **See examples**: Check `NIXPKGS_EXAMPLES.md` for complete file formats
4. **Use templates**: Copy content from the template files

## Files Included

### Documentation

| File | Purpose | Read Time |
|------|---------|-----------|
| **NIXPKGS_CHECKLIST.md** | 📋 Action items and timeline | 5 min |
| **NIXPKGS_SUBMISSION.md** | 📖 Step-by-step detailed guide | 15 min |
| **NIXPKGS_EXAMPLES.md** | 📝 Complete file examples | 10 min |
| **README.md** (this file) | 🎯 Quick navigation | 2 min |

### Templates & Examples

| File | Use For | Copy To |
|------|---------|---------|
| **nixpkgs-default.nix** | Package definition | `pkgs/applications/terminal-ui/kanban/default.nix` |
| **MAINTAINER_TEMPLATE.nix** | Maintainer registration | `maintainers/maintainer-list.nix` |
| **PR_DESCRIPTION_TEMPLATE.md** | Pull request body | GitHub PR form |

## Current Project Status

### ✅ Completed

- [x] Rust code is production-ready (v0.1.10)
- [x] Cargo.toml with complete metadata
- [x] Cargo.lock for reproducible builds
- [x] Apache-2.0 license (nixpkgs-compatible)
- [x] README with features and usage
- [x] flake.nix for development
- [x] Clean git history and meaningful commits
- [x] No native dependencies (pure Rust)
- [x] Cross-platform compatible

### ⏳ Next Steps

- [x] Fork NixOS/nixpkgs repository
- [ ] Register as maintainer
- [ ] Create package definition
- [ ] Calculate hashes
- [ ] Local testing
- [ ] Submit pull request
- [ ] Respond to review feedback

## Reading Path

### For the Impatient (15 minutes)

```
1. NIXPKGS_CHECKLIST.md (5 min)
2. NIXPKGS_EXAMPLES.md - Section 1-3 (5 min)
3. NIXPKGS_SUBMISSION.md - Steps 1-2 (5 min)
```

### For the Thorough (45 minutes)

```
1. NIXPKGS_CHECKLIST.md (complete)
2. NIXPKGS_SUBMISSION.md (all steps)
3. NIXPKGS_EXAMPLES.md (complete)
4. Reference PR_DESCRIPTION_TEMPLATE.md
```

### For Implementation (ongoing)

Follow `NIXPKGS_SUBMISSION.md` in order, referring to:
- `NIXPKGS_EXAMPLES.md` for file format
- `MAINTAINER_TEMPLATE.nix` for registration
- `nixpkgs-default.nix` for package definition
- `PR_DESCRIPTION_TEMPLATE.md` for PR text

## Timeline Estimate

| Phase | Task | Duration |
|-------|------|----------|
| 1 | Fork nixpkgs & create branch | 5 min |
| 2 | Register as maintainer | 5 min |
| 3 | Create package files | 15 min |
| 4 | Calculate hashes | 10 min |
| 5 | Test build | 10 min |
| 6 | Commit & push | 5 min |
| 7 | Create PR | 10 min |
| **Total** | | **60 min** |

## What Makes This Easy

Your project has exceptional preparation:

1. **No native dependencies** - Pure Rust application
2. **Apache-2.0 license** - Widely accepted
3. **Committed Cargo.lock** - Reproducible builds
4. **Clear metadata** - Complete Cargo.toml
5. **Good documentation** - Clear README
6. **Workspace structure** - Clean organization
7. **Release tags** - Proper versioning

**Result**: Straightforward nixpkgs submission with minimal friction.

## Common Questions

### Q: Do I need to build locally first?
**A:** You already have everything built! The nixpkgs build will use Cargo.lock.

### Q: What are the hash values?
**A:** SHA256 hashes of the source and Cargo dependencies. The guide shows how to calculate them.

### Q: How long does review take?
**A:** Typically 3-7 days. ofborg (automated system) runs checks within minutes.

### Q: Can I update the package later?
**A:** Yes! The `nix-update-script` in the package definition helps with updates.

### Q: What if the build fails?
**A:** The guide has a troubleshooting section. Most issues are hash-related.

## After Successful Submission

Once merged to nixpkgs:

```bash
# Users can install with:
nix shell nixpkgs#kanban

# Or on NixOS:
environment.systemPackages = [ pkgs.kanban ];
```

Update your README to mention this!

## Support While Working

If you get stuck:

1. **Check NIXPKGS_EXAMPLES.md** - Has complete working examples
2. **Review NIXPKGS_SUBMISSION.md** - Step-by-step with explanations
3. **Search nixpkgs** - Look at similar Rust CLI packages
4. **Ask community** - #nixos on Libera.Chat IRC

## File Structure

```
kanban/
├── NIXPKGS_SUBMISSION.md      ← Start here for detailed steps
├── NIXPKGS_CHECKLIST.md       ← Overview and action items
├── NIXPKGS_EXAMPLES.md        ← Complete file examples
├── NIXPKGS_README.md          ← This file
├── nixpkgs-default.nix        ← Template package definition
├── MAINTAINER_TEMPLATE.nix    ← Template maintainer entry
├── PR_DESCRIPTION_TEMPLATE.md ← Ready-to-use PR text
├── Cargo.toml
├── Cargo.lock
├── flake.nix
├── default.nix                ← Development package
├── README.md
└── ... (rest of codebase)
```

## Next Action

**→ Open NIXPKGS_CHECKLIST.md and follow the phases**

Good luck! 🚀

---

**Questions?** Check the references in NIXPKGS_SUBMISSION.md
