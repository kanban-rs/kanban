# Packaging

Source files for the OS/distribution packages that ship `kanban` and
`kanban-mcp`. Each subdirectory is one packager; packing and publishing
are automated from `.github/workflows/release.yml`.

| Package | Target | Source |
|---|---|---|
| AUR | Arch Linux | [`aur/`](aur/README.md) |
| Chocolatey | Windows | [`chocolatey/`](chocolatey/README.md) |

See each subdirectory's `README.md` for the per-package release flow and
local smoke-test steps. For release-time failures, the diagnosis and
manual recovery steps live in
[../docs/release-recovery.md](../docs/release-recovery.md).
