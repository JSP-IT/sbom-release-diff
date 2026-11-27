# Changelog

All notable changes are listed here. The project follows
[Semantic Versioning](https://semver.org/). The JSON report has its own
version (`report_version`); see the README.

## [0.1.0] - 2026-11-27

First release.

### Added

- Compare two CycloneDX JSON SBOMs (`specVersion` 1.4–1.7) given as explicit
  file paths.
- Deterministic reporting of added, removed, version-changed and
  declared-license-changed components, including nested components.
- Identity by normalized package URL, with a fallback of type, group and name
  when there is no purl. Ambiguous pairings (duplicate versions with
  different details, several candidate versions, or a purl on one side only)
  are reported instead of guessed.
- SPDX license expression normalization: operand order, identifier case and
  `license.id` vs `expression` do not count as changes. Concluded licenses
  are ignored.
- Text, GitHub-flavored Markdown and versioned JSON output
  (`report_version` 1, with a JSON Schema).
- `--fail-on` policies (`added`, `removed`, `version-changed`,
  `license-changed`, `ambiguous`, `warning`, `any-change`) and stable exit
  codes `0`/`1`/`2`.
- Hardened input handling: size limit, regular files only, strict JSON
  (no repeated keys or trailing data), field validation, component count
  limit.
- Output hardening: terminal control and bidi characters escaped, Markdown
  values confined to code spans, purl credentials and location qualifiers
  never printed.

[0.1.0]: https://github.com/JSP-IT/sbom-release-diff/releases/tag/v0.1.0
