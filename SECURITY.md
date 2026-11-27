# Security

## Reporting a vulnerability

Please report vulnerabilities privately through
[GitHub security advisories](https://github.com/JSP-IT/sbom-release-diff/security/advisories/new).
Do not open a public issue. Include the version, the platform and, if
possible, a minimal SBOM that reproduces the problem. Use synthetic data and
never attach a real SBOM that contains internal information.

Only the latest release is supported with fixes.

## Threat model

SBOMs are treated as **untrusted input**. They may come from third-party
builds, from forks in pull requests, or be crafted to attack whoever reads
the report. The tool is designed so that a hostile SBOM can at worst produce
a wrong or ambiguous comparison, never code execution, resource exhaustion,
terminal or Markdown injection, or leaked credentials.

| Risk | Mitigation |
| --- | --- |
| Oversized or endless input (including `/dev/zero`, pipes, devices) | Only regular files are accepted, checked before and after opening. The size limit (`--max-input-bytes`, default 64 MiB, hard cap 512 MiB) is checked from metadata and enforced again while reading. A parsed document takes several times its file size in memory, so lower the limit on memory-constrained runners. |
| Deep nesting, huge arrays | JSON nesting is limited to 128 levels and documents to 250,000 components. License expressions longer than 4096 bytes or nested more than 32 parentheses deep are not parsed. |
| Parser differentials | Repeated object keys, trailing data, invalid UTF-8 and empty files are rejected, so the tool cannot read a different document than another parser would. |
| Terminal escape injection (ANSI CSI/OSC, C1 controls, bidi overrides, zero-width characters, injected newlines) | All SBOM values in text and Markdown output, and the path and message in errors, are escaped as `\u{XXXX}`. JSON output escapes the same characters as `\uXXXX`. |
| Markdown or HTML injection in PR comments (links, images, `@mentions`, table breaking) | Every SBOM value in Markdown is inside a code span with a fence longer than any backtick run in the value, and pipes are escaped. |
| Credential leakage | Purl location qualifiers (`repository_url`, `download_url`, `vcs_url`, `file_name`, `checksum(s)`) are dropped; `user:password@` in other qualifier values is redacted. Properties, external references, hashes and other fields are never output. Error messages never include SBOM content. Reports contain input file names, not directory paths. |
| Unsafe paths | Inputs must be explicitly named regular files. The tool writes nothing to disk; the report goes to standard output. |
| Network and supply chain | No network access at run time. Three direct dependencies, all permissively licensed and checked by `cargo deny` in CI. `#![forbid(unsafe_code)]` throughout. |

## Out of scope

- Whether the SBOM itself is accurate or complete. The tool compares what the
  documents say.
- Vulnerability data. The tool does not query advisory databases.
- License compatibility or legal advice.
