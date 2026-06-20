# AUR package source

This directory holds the source files for the `kanban`
[AUR](https://aur.archlinux.org/packages/kanban) package: the `PKGBUILD`
and the generated `.SRCINFO`. Bumping and publishing are automated in
`.github/workflows/release.yml` — a local Arch machine isn't required to
cut a release.

## How releases publish

On a release, the `aur` job does the equivalent of the following. It
bumps the version, rewrites the source checksum, regenerates `.SRCINFO`,
and pushes to the AUR via
[`KSXGitHub/github-actions-deploy-aur`](https://github.com/KSXGitHub/github-actions-deploy-aur):

    VERSION='0.7.2'                                         # the version being released
    URL="https://github.com/fulsomenko/kanban/archive/v$VERSION.tar.gz"
    SHA256=$(curl -fsL "$URL" | sha256sum | cut -d' ' -f1)

    sed -i "s|^pkgver=.*|pkgver=$VERSION|" ./PKGBUILD
    sed -i "s|^pkgrel=.*|pkgrel=1|"        ./PKGBUILD
    sed -i "s|sha256sums=.*|sha256sums=(\"$SHA256\")|" ./PKGBUILD
    makepkg --printsrcinfo > .SRCINFO       # regenerate, never hand-edit

`pkgver` and `sha256sums` track the GitHub source tarball; `pkgrel` is
reset to `1` on every version bump. The `.SRCINFO` file is **generated**
from the `PKGBUILD` — do not edit it by hand; run `makepkg --printsrcinfo`
instead.

## Local smoke test (Arch Linux)

    cd packaging/aur
    makepkg -si        # build + install
    kanban --version
    kanban-mcp --version

No Arch box? The CI `.SRCINFO` step runs `makepkg` inside
`nix shell nixpkgs#pacman`, which you can reproduce locally to regenerate
`.SRCINFO` without a full build.

When the `aur` publish step fails, see
[../../docs/release-recovery.md](../../docs/release-recovery.md) for the
manual recovery path. The AUR repo lives at
`ssh://aur@aur.archlinux.org/kanban.git`, separate from this mirror.
