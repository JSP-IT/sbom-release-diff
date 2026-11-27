# sbom-release-diff

Compare the CycloneDX JSON SBOMs of two releases and see which components
were added, removed, changed version or changed their declared licenses.

```console
$ sbom-release-diff release-1.0.0.cdx.json release-1.1.0.cdx.json
...
Summary: 1 added, 1 removed, 6 version changed, 1 license changed, 1 ambiguous, 1 unchanged, 0 warnings

Added (1):
  + pkg:npm/zod  3.23.8  [MIT]

Removed (1):
  - pkg:npm/moment  2.29.4  [MIT]

Version changed (6):
  ~ pkg:npm/lodash  4.17.20 -> 4.17.21
  ...
License changed (1):
  ! pkg:npm/sharp  0.32.6 -> 0.33.4: [Apache-2.0] -> [Apache-2.0 AND LGPL-3.0-or-later]

Ambiguous (1):
  ? name:openssl  same name, but a purl is missing on at least one side
```

The full output is in the [synthetic release comparison](docs/example-release-comparison.md).

- **Deterministic.** The same two inputs always give byte-identical output,
  whatever the order of components, license entries or JSON keys.
- **Conservative matching.** Components are paired by package URL (purl), or by
  type, group and name when there is no purl. If a pairing would be a guess,
  the components are reported as *ambiguous* instead.
- **Offline.** It reads the two files you name and nothing else: no package
  downloads, no vulnerability databases, no telemetry.
- **CI-ready.** Text, GitHub-flavored Markdown and versioned JSON output, plus
  `--fail-on` policies with stable exit codes.

It does **not** judge license compatibility or legal obligations. It only
reports that a component's declared licenses are written differently. Whether
a change matters is for you to decide.

## Install

Download an archive for your platform from the
[releases page](https://github.com/JSP-IT/sbom-release-diff/releases) and
verify it against `checksums.txt`:

```sh
sha256sum --check --ignore-missing checksums.txt
```

Or build from source (Rust 1.85 or later):

```sh
cargo install --locked --git https://github.com/JSP-IT/sbom-release-diff --tag v0.1.0
```

Builds are provided for Linux (static musl), macOS and Windows, on x86_64 and
arm64. The crate is not published to crates.io.

## Usage

```text
sbom-release-diff [OPTIONS] <OLD> <NEW>

Arguments:
  <OLD>  CycloneDX JSON SBOM of the earlier release
  <NEW>  CycloneDX JSON SBOM of the later release

Options:
  -f, --format <FORMAT>        text (default), markdown or json
      --fail-on <POLICY>       exit 1 when any of these occur (comma-separated or repeated):
                               added, removed, version-changed, license-changed,
                               ambiguous, warning, any-change
      --max-input-bytes <N>    reject an input larger than N bytes
                               (default 67108864 = 64 MiB, at most 536870912 = 512 MiB)
  -h, --help / -V, --version
```

Both inputs must be regular files named on the command line. Standard input,
directories, globs and URLs are not accepted. The report goes to standard
output and errors go to standard error.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Report written; no `--fail-on` policy was violated. |
| `1` | Report written; at least one `--fail-on` policy was violated. |
| `2` | Usage error, unreadable, invalid, unsupported or oversized input, or the report could not be written. No report is written. |

`any-change` covers added, removed, version-changed, license-changed and
ambiguous. It does not cover warnings; use `warning` for those.

### Output formats

- **text**: for terminals and CI logs.
- **markdown**: for pull request comments and `$GITHUB_STEP_SUMMARY`. Every
  value from the SBOM is inside a code span, so it cannot inject links, HTML,
  `@mentions` or table cells.
- **json**: for tooling. The format is versioned (`"report_format":
  "sbom-release-diff-report"`, `"report_version": 1`) and described by
  [schema/report-v1.schema.json](schema/report-v1.schema.json). Within version 1
  fields may be added, but none are removed, renamed or given a new meaning.
  Arrays are sorted, optional values are `null`, and only the file name of each
  input is included, not its directory.

## What is compared

| Report section | Meaning |
| --- | --- |
| Added / Removed | An identity is present on one side only. |
| Version changed | A paired component's version string differs. Versions are compared as exact strings: `1.0` → `1.0.0` is a change, and no upgrade or downgrade direction is claimed. |
| License changed | A paired component's set of declared licenses differs after normalization (see below). A component can appear under both version and license changes. |
| Ambiguous | Components that cannot be paired without guessing. |
| Warnings | Merged exact duplicates, purl/version mismatches and license expressions that could not be parsed. |

Only `components` (including nested `components`) are compared.
`metadata.component`, the thing the SBOM describes, is shown as the
*subject* but not compared. Services, dependencies, vulnerabilities, hashes,
properties, external references and evidence are ignored and never appear in
the output.

### Identity and matching

In short:

1. **purl identity.** The purl with its version removed, type-normalized,
   qualifiers sorted, and location or integrity qualifiers (`repository_url`,
   `download_url`, `vcs_url`, `file_name`, `checksum`, `checksums`) dropped.
2. **Fallback identity** when there is no purl: component `type` + `group` +
   `name`, exactly as written. `bom-ref` is never used, because it is only
   unique within one document.
3. Within an identity, entries are paired by exact version first. Leftovers
   are paired only if each side had exactly one entry. Otherwise they are
   reported as ambiguous.
4. A removed and an added component with the same group and name, where at
   least one of them has no purl, are reported as ambiguous
   (`identity-basis-differs`) instead of as a separate removal and addition.

The rules, the normalizations and their limitations are in
[docs/identity.md](docs/identity.md).

### Declared licenses

The `licenses` array of each component is compared as a set of entries:

- `license.id` is compared case-insensitively.
- `license.name` is compared exactly, after trimming and collapsing whitespace.
- `expression` is parsed as an SPDX expression. Identifiers are lowercased,
  and `AND`/`OR` operands are flattened, deduplicated and sorted, so
  `MIT OR Apache-2.0` equals `(Apache-2.0 OR MIT)`. Operators must be
  uppercase, as SPDX requires. An expression that does not parse is compared
  as whitespace-normalized text and produces an `unparsed-license-expression`
  warning.
- `license.id: MIT` and `expression: MIT` are the same declaration.
- Entries with `"acknowledgement": "concluded"` (CycloneDX 1.6+) are skipped,
  because only declared licenses are compared.

No license list is consulted, no SPDX ID is validated, and nothing is said
about compatibility.

## Supported CycloneDX versions

| `specVersion` | Supported |
| --- | --- |
| 1.4, 1.5, 1.6, 1.7 | Yes (JSON) |
| 1.3 and earlier, anything else | Rejected with exit code 2 |
| XML or Protocol Buffers encodings | Not supported; convert to JSON first |
| SPDX | Not supported |

The two inputs may use different supported versions. The tool validates the
fields it reads, not the whole CycloneDX schema. If you need full schema
validation, run a schema validator such as `cyclonedx validate` first.

## Input validation and safety

Inputs are untrusted. The tool:

- accepts only regular files, at most `--max-input-bytes` each (checked before
  and while reading), with at most 250,000 components and 128 levels of JSON
  nesting;
- rejects invalid JSON, invalid UTF-8, trailing data, empty files and objects
  with **repeated keys** (different parsers disagree on which value wins);
- requires `bomFormat: "CycloneDX"`, a supported `specVersion`, and for every
  component a string `type` and `name`, correctly typed optional fields, a
  well-formed purl if one is present, and well-formed license entries;
- never copies SBOM values into error messages; errors name only a location
  such as `components[3].purl`;
- escapes control characters (including ANSI/OSC escape sequences and C1
  controls), bidirectional overrides and zero-width characters in text and
  Markdown output, and truncates long values to 200 characters there. JSON
  output escapes the same characters as `\uXXXX`, so the data is unchanged;
- drops location qualifiers from purls and replaces `user:password@` in any
  remaining URL-like qualifier with `REDACTED@`;
- reads no environment variables or configuration files, makes no network
  connections and writes no files.

See [SECURITY.md](SECURITY.md) for the threat model and how to report a
vulnerability.

## CI

```yaml
- name: Compare SBOMs
  run: |
    sbom-release-diff previous.cdx.json current.cdx.json \
      --format markdown --fail-on removed,license-changed,ambiguous \
      >> "$GITHUB_STEP_SUMMARY"
```

More examples, including GitHub Actions with a pinned checksum, GitLab CI and
using the JSON report with `jq`, are in [docs/ci.md](docs/ci.md).

## Compared with other tools

[sbom-diff](https://crates.io/crates/sbom-diff),
[sbom-tools](https://crates.io/crates/sbom-tools) and the `diff` command of
[cyclonedx-cli](https://github.com/CycloneDX/cyclonedx-cli) also compare
SBOMs, and they support more formats. sbom-release-diff is deliberately
narrower:

- it reports ambiguity instead of falling back to name-only or fuzzy matching;
- its JSON report has a schema with compatibility rules;
- it has hard input limits, strict JSON parsing and output sanitization;
- it never touches the network.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo deny check            # optional locally; runs in CI
```

Fixtures live in [tests/fixtures](tests/fixtures), and the golden outputs of
the example comparison in [tests/expected](tests/expected). If you change the
output on purpose, regenerate the golden files and review the diff. The
dependency license review is in [docs/dependencies.md](docs/dependencies.md)
and the release process in [RELEASING.md](RELEASING.md).

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
