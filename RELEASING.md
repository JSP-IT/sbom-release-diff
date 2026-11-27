# Releasing

Releases are cut from `main`. Release binaries are built by the
**Release binaries** workflow, which only uploads assets to an existing
**draft** release and never publishes it. Nothing is published to crates.io.

## Repository protections

Two rulesets (Settings → Rules → Rulesets) protect the release path. Neither
has bypass actors, so the rules also apply to repository admins.

**Protect main** (the default branch):

- Changes land only through pull requests, and every review thread must be
  resolved before merging. No approving review is required, because the
  repository has a single maintainer account, which cannot approve its own
  pull requests.
- Force pushes and deleting the branch are blocked.
- These CI checks must pass before merging: `fmt and Clippy`,
  `dependency licenses and advisories`, `binaries / checksums`, and
  `test (<os>, <toolchain>)` for `ubuntu-latest`, `windows-latest` and
  `macos-latest` with `stable` and `1.85`.

Required checks are matched by job name. If you rename a CI job or change the
test matrix (for example when raising the MSRV), update the ruleset in the
same change. Otherwise pull requests wait forever for a check that no longer
runs.

**Protect release tags** (`refs/tags/v*`):

- Release tags can be created, but not moved, force-pushed or deleted. A tag
  therefore always points at the commit the release workflow validated and
  built.
- If a tag is pushed on the wrong commit, it cannot be fixed in place. If no
  release has been published for it, an admin can temporarily disable the
  ruleset, delete the tag and turn the ruleset back on. Once a release is
  published, cut a new patch version instead.

## Labels and release notes

Label every pull request before merging. GitHub groups the generated release
notes by these labels (see `.github/release.yml`):

| Label | Section |
| --- | --- |
| `breaking-change` | Breaking changes |
| `security` | Security |
| `enhancement` | Features |
| `bug` | Bug fixes |
| `documentation` | Documentation |
| `ci` | CI and release tooling |
| `dependencies` | Dependencies |
| `skip-changelog` | Left out of the notes |

Pull requests that only prepare a release also get the `release` label.

A change to the meaning or shape of an existing JSON report field needs a new
`report_version`, a new schema file, and a `breaking-change` label.

## Steps

1. Update `version` in `Cargo.toml` (and `Cargo.lock` with `cargo update -p
   sbom-release-diff`), move the `CHANGELOG.md` entry from "unreleased" to
   the release date, and write `RELEASE_NOTES_vX.Y.Z.md`. Merge the release
   pull request into `main` once a maintainer has reviewed it; the ruleset
   blocks the merge until the required checks pass.
2. Tag the merge commit and push the tag. A draft release does not create its
   tag, so the tag must exist before the workflow runs. Check the commit
   first: release tags cannot be moved or deleted once pushed (see
   [Repository protections](#repository-protections)).

   ```sh
   git switch main
   git pull --ff-only
   git tag -a vX.Y.Z -m "sbom-release-diff vX.Y.Z"
   git push origin vX.Y.Z
   ```

3. Create a draft release for the existing tag. `--generate-notes` appends the
   labelled pull request list to the hand-written notes:

   ```sh
   gh release create vX.Y.Z --draft --verify-tag \
     --title "sbom-release-diff vX.Y.Z" \
     --notes-file RELEASE_NOTES_vX.Y.Z.md --generate-notes
   ```

4. Run the workflow and wait for it to finish:

   ```sh
   gh workflow run release.yml -f tag=vX.Y.Z
   ```

   It checks that the tag is on `main`, matches `Cargo.toml` and has a draft
   release. It runs fmt, Clippy and the tests at the tag, and builds archives
   for Linux (musl), macOS and Windows on x86_64 and arm64. Each archive
   contains the binary, `LICENSE`, `NOTICE`, `README.md`, `CHANGELOG.md` and
   `THIRD-PARTY-LICENSES.txt`. The workflow verifies `checksums.txt` and
   uploads everything to the draft. If the release is no longer a draft, it
   refuses to upload.

5. Review the draft. Download one archive and check it:

   ```sh
   gh release download vX.Y.Z --pattern 'checksums.txt' --pattern '*x86_64-unknown-linux-musl*'
   sha256sum --check --ignore-missing checksums.txt
   tar -xzf sbom-release-diff_vX.Y.Z_x86_64-unknown-linux-musl.tar.gz
   ./sbom-release-diff_vX.Y.Z_x86_64-unknown-linux-musl/sbom-release-diff --version
   ```

6. Publish by hand once you are satisfied:

   ```sh
   gh release edit vX.Y.Z --draft=false --latest
   ```

Every pull request also builds the same archives with `checksums.txt` as
workflow artifacts (see the `binaries` job in CI), so reviewers can try a
change before it is merged. Those artifacts are not release assets.
