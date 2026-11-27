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

_sbom-release-diff {{TOOL_VERSION}}, report v1. Declared licenses are compared as written; this is not a license compatibility assessment._
