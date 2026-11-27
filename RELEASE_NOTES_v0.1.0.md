# sbom-release-diff v0.1.0

First release of sbom-release-diff, an offline Rust CLI that compares the
CycloneDX JSON SBOMs of two releases and reports component changes for
review and CI gates.

## Supported

- CycloneDX JSON with `specVersion` 1.4, 1.5, 1.6 and 1.7. The two inputs
  may use different versions. They are given as explicit file paths.
- Added, removed, version-changed and declared-license-changed components,
  nested components included, in deterministic order.
- Identity by normalized package URL, without version or location
  qualifiers. Components without a purl fall back to type, group and name.
- Ambiguous matches are reported, not guessed: the same version listed twice
  with different details, several candidate versions on both sides, or a
  component that has a purl on one side only.
- Declared licenses are compared after SPDX expression normalization, so
  `MIT OR Apache-2.0` equals `Apache-2.0 OR MIT`. Concluded licenses are
  ignored.
- Text, Markdown (for PR comments and job summaries) and JSON output. JSON
  follows `report_version` 1 and has a published schema.
- `--fail-on added,removed,version-changed,license-changed,ambiguous,warning,any-change`.
  Exit codes: `0` passed, `1` policy violated, `2` usage or input error.
- Inputs that are malformed, unsupported or oversized are rejected: invalid
  JSON, repeated keys, trailing data, wrong field types, malformed purls,
  more than 64 MiB by default, or more than 250,000 components.
- Output is hardened against terminal escape and Markdown injection. Purl
  credentials and location qualifiers are never printed.
- Release archives for Linux (static musl), macOS and Windows on x86_64 and
  arm64, with SHA-256 checksums and third-party license notices.

## Install

Download an archive below and verify it:

```sh
sha256sum --check --ignore-missing checksums.txt
```

Or build from source with Rust 1.85 or later:

```sh
cargo install --locked --git https://github.com/JSP-IT/sbom-release-diff --tag v0.1.0
```

## Known limitations

- CycloneDX XML and Protocol Buffers, and SPDX, are not supported. Convert
  to CycloneDX JSON first.
- Versions are compared as exact strings, with no ecosystem-specific
  ordering, so no upgrade or downgrade is claimed.
- Purl normalization covers the generic rules plus `github`, `bitbucket` and
  `pypi`. Other ecosystem-specific case rules are not applied.
- Platform qualifiers such as `arch` and `distro` are part of the identity,
  so changing one shows up as a removal plus an addition.
- Renamed or moved packages are reported as a removal plus an addition.
- `metadata.component` is shown as the subject but not compared.
- Only the fields the tool reads are validated, not the full CycloneDX
  schema.
- The tool reports differences in declared licenses. It does not assess
  license compatibility or give legal advice, and it does not fetch packages
  or query vulnerability databases.
