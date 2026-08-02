# Chocolatey package source

This directory holds the source files for the `kanban` Chocolatey
package. Pack and publish are automated in
`.github/workflows/release.yml` — local Windows isn't required.

The CI flow (publish-chocolatey job) does the equivalent of the
following PowerShell. Both `$VERSION` and `$SHA` are real PowerShell
variables; set them before running the rest:

    # In PowerShell on a Windows machine:
    $VERSION = '0.6.0'                                       # the version being packed
    $SHA     = (Get-FileHash -Algorithm SHA256 `
                  "kanban-v$VERSION-x86_64-pc-windows-msvc.zip").Hash.ToLower()

    # Substitute placeholders into a temp build directory.
    Copy-Item kanban.nuspec build/kanban.nuspec
    Copy-Item tools/* build/tools/ -Recurse
    (Get-Content build/kanban.nuspec) -replace '\$version\$', $VERSION | Set-Content build/kanban.nuspec
    (Get-Content build/tools/chocolateyinstall.ps1) `
      -replace '\$version\$',   $VERSION `
      -replace '\$checksum64\$', $SHA `
      | Set-Content build/tools/chocolateyinstall.ps1
    choco pack                                  # in build/
    choco install kanban -s . -y --force        # smoke test
    kanban --version
    kanban-mcp --version
    choco push kanban.$VERSION.nupkg ...

To re-run the smoke locally on a Windows box, copy the build/ steps above.
Do NOT commit substituted files.

When the `publish-chocolatey` workflow job fails, see [RECOVERY.md](RECOVERY.md)
for the diagnosis flowchart. Chocolatey rejects re-pushing the same
`id + version` permanently, so the recovery path differs sharply by failure
mode and is not safe to guess.
