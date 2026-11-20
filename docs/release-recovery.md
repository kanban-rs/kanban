# Release recovery runbook

When `.github/workflows/release.yml` fails partway through, this runbook
tells you how to finish the release. Since the per-job restructure, the
first move is almost always a re-run from the GitHub Actions UI; the
manual fallbacks at the bottom of each section are the last resort.

## Pipeline shape

The workflow is split into per-leg jobs so a failure in one leg never
forces you to redo (or hand-finish) the others:

```
preflight -> prepare -> publish-crates -> tag-release -+-> publish-aur -> sync-develop
                                                       +-> publish-homebrew
                                                       +-> build-windows -> publish-chocolatey
                                                                         -> publish-winget
```

- **preflight** is mutation-free. It determines the run mode and probes
  every credential the pipeline will need (`DEPLOY_KEY`,
  `CARGO_REGISTRY_TOKEN`, `AUR_SSH_KEY`, `HOMEBREW_TAP_DEPLOY_KEY`,
  `WINGET_TOKEN`, `CHOCO_API_KEY`), each in its own step so a failure
  names the broken credential. If preflight fails, nothing has been
  pushed, published, or tagged; fix the credential and re-run.
- **prepare** is the only job that builds the release commit
  (version bump, changelog aggregation, changeset deletion) and pushes
  it to `master`.
- Every downstream leg checks out `master` fresh and consumes
  `needs.preflight.outputs.version`, so each leg can re-run on its own.

## Run modes

Preflight classifies the run and exposes it as `mode`:

- **release**: changesets exist on `master`. The normal path; `prepare`
  runs and the pipeline proceeds end to end.
- **resume**: no changesets, but the version in `master`'s `Cargo.toml`
  has no corresponding `v{X}` tag. This is the signature of a run that
  died after `prepare` pushed the release commit. `prepare` is skipped
  and the pipeline resumes at `publish-crates`.
- **noop**: no changesets and the tag exists. Every downstream job
  skips. This keeps non-release merges to `master` (and full re-runs of
  an already-tagged release) green without side effects.

## Recovery, in order of preference

1. **"Re-run failed jobs"** from the workflow run page. Each leg is its
   own job, so only the failed leg (and anything downstream of it)
   re-runs; outputs of the successful jobs are preserved. Every leg is
   idempotent: crates.io publishing skips already-published crates, the
   tag step skips an existing tag, the GitHub Release action updates in
   place, and the AUR/Homebrew bumps skip when there is no diff.
2. **"Re-run all jobs"** when the run died between `prepare`'s push to
   `master` and the tag push. Preflight detects resume mode (consumed
   changesets, untagged version) and completes every remaining leg.
3. **`workflow_dispatch` with the `version` input** re-runs only the
   Windows legs (`build-windows`, `publish-chocolatey`,
   `publish-winget`) for an already-tagged version, e.g. when the
   original run is too old to re-run.
4. **Manual fallbacks** below, when a re-run does not converge.

**Caveat:** once the tag exists, a full "Re-run all jobs" is a noop by
design (preflight sees the tag and classifies the run as noop). For a
failure in any job after `tag-release`, use "Re-run failed jobs" on the
original run, or the `workflow_dispatch` path for the Windows legs.

## Job: prepare (release commit and push to master)

**Symptom:** version bump, changelog, commit, validation, or the push
to `master` failed.

**State on origin:** no release commit, no tag, nothing published
(preflight already proved the credentials work, so a push failure here
is something new, e.g. branch protection or a force-push race).

**Recovery:** re-run failed jobs. The changesets are still on origin,
so `prepare` re-produces the same release commit and retries the push.
If `master` already carries the release commit (a re-run race), the
push step detects the version match and skips instead of failing.
If it still fails, nothing is partially shipped; it is safe to revert
the merged PR, fix the underlying issue, and re-merge.

## Job: publish-crates

**Symptom:** `nix run .#publish-crates` exited non-zero partway
through the crate list.

**State on origin:** release commit pushed; no tag; some crates may be
on crates.io already.

**Recovery:** re-run failed jobs. `scripts/publish-crates.sh` is
idempotent per crate: `cargo publish` failures whose output contains
"already exists" are skipped, so a re-run converges after a partial
publish.

If you must finish by hand:
```bash
export CARGO_REGISTRY_TOKEN=<token>
git fetch origin && git checkout master && git pull --ff-only
nix run .#publish-crates
```

## Job: tag-release (tag and GitHub Release)

**Symptom:** the tag push or the GitHub Release creation failed.

**State on origin:** release commit and crates.io are at the new
version; tag and Release object may or may not exist.

**Recovery:** re-run failed jobs. The tag step guards both the local
tag creation and the push against an existing tag, and
`softprops/action-gh-release@v2` creates the Release if missing and
updates it if present.

If you must finish by hand:
```bash
git fetch origin && git checkout master && git pull --ff-only
git tag "v<VERSION>"
git push origin "v<VERSION>"
gh release create "v<VERSION>" --generate-notes
```

## Job: publish-aur

**Symptom:** the PKGBUILD bump, the commit to `master`, or the AUR
deploy failed.

**State on origin:** crates.io, tag, and GitHub Release are at the new
version; AUR may have a stale `pkgver`.

**Recovery:** re-run failed jobs. The job re-checks out `master`; if a
previous attempt already committed the PKGBUILD bump, the sed produces
no diff and the commit step skips, then the deploy action re-runs
(`allow_empty_commits: false` keeps that safe).

If you must finish by hand:
```bash
git clone ssh://aur@aur.archlinux.org/kanban.git /tmp/aur-kanban
cd /tmp/aur-kanban
# Edit pkgver, pkgrel=1, sha256sums in PKGBUILD
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO
git commit -m "Update to <VERSION>"
git push
```

## Job: publish-homebrew

**Symptom:** the formula bump or push to `fulsomenko/homebrew-tap`
failed.

**State on origin:** everything upstream of this leg is at the new
version; the tap formula may be stale.

**Recovery:** re-run failed jobs. The bump step skips the commit when
the formula already matches the target (no diff), so a re-run is safe.

If you must finish by hand:
```bash
git clone git@github.com:fulsomenko/homebrew-tap.git /tmp/homebrew-tap
cd /tmp/homebrew-tap
# Edit Formula/kanban.rb: url, sha256, version
git add Formula/kanban.rb
git commit -m "Bump kanban to <VERSION>"
git push
```

## Job: sync-develop

**Symptom:** the merge of `master` into `develop` failed (the job
fails loudly instead of force-resolving).

**Recovery:** merge by hand and push:
```bash
git fetch origin
git checkout develop && git pull origin develop
git merge origin/master   # resolve conflicts
git push origin develop
```

Note: this job runs after `publish-aur` so the AUR bump commit on
`master` is included in the sync. If `publish-aur` failed and was
skipped past, develop trails `master` by that commit until the AUR
job is re-run or the merge is done by hand.

## Job: build-windows

**Symptom:** the Windows build, archive, or asset upload failed.

**State on origin:** the tag and Release exist (the job depends on
`tag-release`); the Windows ZIP and SHA256SUMS may be missing from the
Release.

**Recovery:** re-run the `build-windows` job from the UI. It re-checks
out the tag, rebuilds, and re-uploads; `softprops/action-gh-release@v2`
updates assets in place. If the original run is gone, use
`workflow_dispatch` with the released version.

## Job: publish-chocolatey

**Symptom:** the Chocolatey push failed or was held in the moderation
queue.

The job is marked `continue-on-error: true`, so this failure surfaces
as a warning rather than blocking the workflow. Look for the yellow
warning banner and the step-summary block on the workflow run page.

**Recovery:** see [`packaging/chocolatey/RECOVERY.md`](../packaging/chocolatey/RECOVERY.md)
for the full Chocolatey-specific flowchart. The short version:
1. The push step pre-checks the public OData API and skips if the
   version is already published, so re-running is safe.
2. If `choco push` itself failed with a hard error, follow the
   chocolatey runbook to either retry, contact moderation, or
   ship the next patch with a corrected nupkg.

## Job: publish-winget

**Symptom:** the winget submission failed. Also
`continue-on-error: true`, surfaced as a warning.

**Recovery:** the most common cause is that the first-ever submission
of a new identifier must be made manually; see
[`packaging/winget/README.md`](../packaging/winget/README.md).
Otherwise re-run the job or use `workflow_dispatch`.
