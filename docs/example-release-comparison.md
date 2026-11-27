# Synthetic release comparison

This walk-through compares two **synthetic** SBOMs for a fictional web shop,
`acme-storefront`, from release 1.0.0 to 1.1.0. The package names are real
open-source packages; the versions, licenses and the `acme` components were
chosen to exercise each kind of report entry. They are not taken from any
real product.

- Old: [examples/release-1.0.0.cdx.json](../examples/release-1.0.0.cdx.json) (CycloneDX 1.5)
- New: [examples/release-1.1.0.cdx.json](../examples/release-1.1.0.cdx.json) (CycloneDX 1.6)

## What changed between the two files

| Component | 1.0.0 | 1.1.0 | Expected report |
| --- | --- | --- | --- |
| `express` | 4.18.2, MIT | 4.19.2, MIT (with `acknowledgement: declared`) | version changed |
| `lodash` | 4.17.20 | 4.17.21 | version changed |
| `moment` | 2.29.4 | (gone) | removed |
| `zod` | (absent) | 3.23.8 | added |
| `react` | 18.2.0, `license.id: MIT` | 18.2.0, `expression: MIT` | unchanged (same declaration) |
| `sharp` | 0.32.6, Apache-2.0 | 0.33.4, `Apache-2.0 AND LGPL-3.0-or-later` | version **and** license changed |
| `libvips` (nested under `sharp`) | 8.14.5 | 8.15.2 | version changed |
| `jackson-databind` (Maven, `?type=jar`) | 2.15.2 | 2.17.1 | version changed |
| `com.acme/acme-internal-utils` (no purl) | 2.3.0 | 2.4.0 | version changed (fallback identity) |
| `openssl` | purl `pkg:generic/openssl@3.0.13` | no purl, 3.0.14 | **ambiguous**: is it the same component? |

The subject (`metadata.component`) goes from `acme-storefront 1.0.0` to
`1.1.0`. It is shown in the header, not compared.

## Text output

```console
$ sbom-release-diff examples/release-1.0.0.cdx.json examples/release-1.1.0.cdx.json
sbom-release-diff 0.1.0 - report v1
Old: release-1.0.0.cdx.json (CycloneDX 1.5, 9 components, subject: acme-storefront 1.0.0)
New: release-1.1.0.cdx.json (CycloneDX 1.6, 9 components, subject: acme-storefront 1.1.0)

Summary: 1 added, 1 removed, 6 version changed, 1 license changed, 1 ambiguous, 1 unchanged, 0 warnings

Added (1):
  + pkg:npm/zod  3.23.8  [MIT]

Removed (1):
  - pkg:npm/moment  2.29.4  [MIT]

Version changed (6):
  ~ com.acme/acme-internal-utils (library, no purl)  2.3.0 -> 2.4.0
  ~ pkg:generic/libvips  8.14.5 -> 8.15.2
  ~ pkg:maven/com.fasterxml.jackson.core/jackson-databind?type=jar  2.15.2 -> 2.17.1
  ~ pkg:npm/express  4.18.2 -> 4.19.2
  ~ pkg:npm/lodash  4.17.20 -> 4.17.21
  ~ pkg:npm/sharp  0.32.6 -> 0.33.4

License changed (1):
  ! pkg:npm/sharp  0.33.4: [Apache-2.0] -> [Apache-2.0 AND LGPL-3.0-or-later]

Ambiguous (1):
  ? name:openssl  same name, but a purl is missing on at least one side
      old: pkg:generic/openssl 3.0.13 [Apache-2.0]
      new: openssl (library, no purl) 3.0.14 [Apache-2.0]

Policy: passed (no --fail-on policy set)
```

Exit code `0`, because no `--fail-on` policy was given.

## With a CI policy

```console
$ sbom-release-diff examples/release-1.0.0.cdx.json examples/release-1.1.0.cdx.json \
    --fail-on removed,license-changed,ambiguous > /dev/null; echo "exit code $?"
exit code 1
```

The last line of the report then reads:

```text
Policy: FAILED: removed, license-changed, ambiguous (fail on: removed, license-changed, ambiguous)
```

A reviewer would look at three things: `moment` was removed (check whether
anything still needs it), `sharp` now declares an extra LGPL license (route it
to whoever reviews licenses), and `openssl` lost its purl, so the tool cannot
confirm that 3.0.14 replaces 3.0.13. The fix for the last one is usually to
correct the SBOM generator so that it emits a purl again.

## Markdown output

`--format markdown` gives the same content for a pull request comment or
`$GITHUB_STEP_SUMMARY`:

````markdown
## SBOM release diff

| | Old | New |
|---|---|---|
| File | ` release-1.0.0.cdx.json ` | ` release-1.1.0.cdx.json ` |
| CycloneDX | ` 1.5 ` | ` 1.6 ` |
| Subject | ` acme-storefront 1.0.0 ` | ` acme-storefront 1.1.0 ` |
| Components | 9 | 9 |

**Summary:** 1 added, 1 removed, 6 version changed, 1 license changed, 1 ambiguous, 1 unchanged, 0 warnings

### Added (1)

| Component | Version | Declared licenses |
|---|---|---|
| ` pkg:npm/zod ` | ` 3.23.8 ` | ` MIT ` |

### Removed (1)

| Component | Version | Declared licenses |
|---|---|---|
| ` pkg:npm/moment ` | ` 2.29.4 ` | ` MIT ` |

### Version changed (6)

| Component | Old version | New version |
|---|---|---|
| ` com.acme/acme-internal-utils (library, no purl) ` | ` 2.3.0 ` | ` 2.4.0 ` |
| ` pkg:generic/libvips ` | ` 8.14.5 ` | ` 8.15.2 ` |
| ` pkg:maven/com.fasterxml.jackson.core/jackson-databind?type=jar ` | ` 2.15.2 ` | ` 2.17.1 ` |
| ` pkg:npm/express ` | ` 4.18.2 ` | ` 4.19.2 ` |
| ` pkg:npm/lodash ` | ` 4.17.20 ` | ` 4.17.21 ` |
| ` pkg:npm/sharp ` | ` 0.32.6 ` | ` 0.33.4 ` |

### Declared license changed (1)

| Component | Version | Old licenses | New licenses |
|---|---|---|---|
| ` pkg:npm/sharp ` | ` 0.33.4 ` | ` Apache-2.0 ` | ` Apache-2.0 AND LGPL-3.0-or-later ` |

### Ambiguous (1)

| Identity | Reason | Old | New |
|---|---|---|---|
| ` name:openssl ` | same name, but a purl is missing on at least one side | ` pkg:generic/openssl ` ` 3.0.13 ` | ` openssl (library, no purl) ` ` 3.0.14 ` |

**Policy:** passed (no --fail-on policy set)

_sbom-release-diff 0.1.0, report v1. Declared licenses are compared as written; this is not a license compatibility assessment._
````

## JSON output

`--format json` gives the full report in a versioned format
([schema](../schema/report-v1.schema.json)). Here is the license change entry:

```json
{
  "identity": "pkg:npm/sharp",
  "identity_basis": "purl",
  "component_type": "library",
  "name": "sharp",
  "group": null,
  "old_version": "0.32.6",
  "new_version": "0.33.4",
  "old_licenses": [
    "Apache-2.0"
  ],
  "new_licenses": [
    "Apache-2.0 AND LGPL-3.0-or-later"
  ]
}
```

and the ambiguous entry:

```json
{
  "identity": "name:openssl",
  "reason": "identity-basis-differs",
  "old": [
    {
      "identity": "pkg:generic/openssl",
      "identity_basis": "purl",
      "component_type": "library",
      "name": "openssl",
      "group": null,
      "version": "3.0.13",
      "licenses": [
        "Apache-2.0"
      ]
    }
  ],
  "new": [
    {
      "identity": "component:library//openssl",
      "identity_basis": "fallback",
      "component_type": "library",
      "name": "openssl",
      "group": null,
      "version": "3.0.14",
      "licenses": [
        "Apache-2.0"
      ]
    }
  ]
}
```

The complete JSON output is in
[tests/expected/release-1.0.0-to-1.1.0.json](../tests/expected/release-1.0.0-to-1.1.0.json)
(with the tool version shown as `{{TOOL_VERSION}}`).
