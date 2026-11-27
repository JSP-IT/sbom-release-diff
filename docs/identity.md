# Component identity and matching

This page describes exactly how sbom-release-diff decides that a component in
the old SBOM and a component in the new SBOM are the same thing, and where
that decision can go wrong.

The guiding rule: **if pairing would be a guess, do not pair.** A false "same
component" hides a change, while "ambiguous" is visible and can be resolved by
a person. The tool therefore prefers an ambiguous result, or a separate
addition and removal, over an uncertain match.

## 1. Identity keys

Every component gets one version-independent identity key.

### Components with a purl

The `purl` field is parsed according to the
[purl specification](https://github.com/package-url/purl-spec):

| Step | Effect |
| --- | --- |
| Scheme | Must be `pkg:` (any case). Leading slashes after it are ignored. |
| Type | Lowercased; must start with a letter and contain only letters, digits, `.`, `+` and `-`. |
| Namespace, name, subpath | Percent-decoded. Subpath segments `.`, `..` and empty segments are dropped. |
| Version | Removed from the key. An `@` counts as the version separator only after the last `/`, so an unencoded npm scope (`pkg:npm/@angular/core`) parses correctly. |
| Qualifiers | Keys lowercased and sorted; empty values dropped; a repeated key is an error. |
| Location qualifiers | `repository_url`, `download_url`, `vcs_url`, `file_name`, `checksum` and `checksums` are dropped. They describe where an artifact came from, not what it is, and may contain credentials. |
| Credentials | In the remaining qualifier values, `scheme://user:password@host` becomes `scheme://REDACTED@host`. |
| Type rules | `github` and `bitbucket`: namespace and name lowercased. `pypi`: name lowercased and `_` replaced with `-`. Other types are compared exactly as decoded. |

The key is then written in an ASCII-only canonical form, for example
`pkg:maven/com.fasterxml.jackson.core/jackson-databind?type=jar`.

A purl that cannot be parsed makes the whole input invalid (exit code 2).

If the purl has a version, that version is the component's version. If the
`version` field disagrees with it, a `purl-version-mismatch` warning is
reported and the purl wins.

### Components without a purl (fallback)

The key is `component:<type>/<group>/<name>`. Each part is percent-encoded,
so a `/` inside a group or name cannot run into the separator. The values are
used **exactly as written**, with no case folding. The version comes from the
`version` field. An empty `version` or `group` string counts as absent.

These are deliberately not used for identity:

- `bom-ref`: unique only inside one document, and often a random UUID.
- `cpe`, `swid`, `hashes`, `author`, `publisher`, `supplier`: they are
  optional, inconsistently populated across generators, or (for hashes)
  change with every version.

## 2. Pairing within an identity

All components, nested ones included, are grouped by identity key on each
side. Within each group:

1. **Exact duplicates are merged.** Entries with the same version, declared
   licenses, name, group and type count once, and a `duplicate-component`
   warning gives the count.
2. **Pair by exact version.** If a version appears once on each side, those
   two entries are paired. If a version appears on both sides but more than
   once on at least one side (with different details), the group is
   `duplicate-version-differs` and nothing in it is paired.
3. **Leftovers:**
   - only on the old side → *removed*;
   - only on the new side → *added*;
   - one on each side, and each side had a single entry for this identity →
     paired, which gives a *version change*;
   - anything else → `multiple-candidates` (ambiguous).

So `lodash 4.17.20 + 4.17.21` → `lodash 4.17.21` reports 4.17.20 as removed.
But `ms 2.0.0 + 2.1.2` → `ms 2.1.2 + 2.1.3` pairs 2.1.2 and reports
`2.0.0 ↔ 2.1.3` as ambiguous: it could be an upgrade, or a removal plus an
unrelated addition.

## 3. Identity basis changes

When the old SBOM has a purl for a component and the new one does not (or the
other way round), the two entries get different keys. On their own they would
show up as a removal and an addition. After pairing, every removed and added
component is checked against a *loose key*, the lowercased `group/name`. If
removed and added components share a loose key and at least one of them has
no purl, they are all reported together as `identity-basis-differs`. They are
taken out of the added and removed lists.

This also catches fallback components whose `type` changed (for example
`library` → `framework`).

If all the components involved have purls, no loose check is applied. Two
different purls are two different packages, even if their names match.

## 4. Limitations

These are known and intentional. Each one fails *visibly*, as added, removed
or ambiguous, never as a silent wrong match.

- **Versions are strings.** `1.0` and `1.0.0` differ, and no ordering is
  applied, so the tool does not say "upgrade" or "downgrade". The version
  schemes of the different ecosystems are not modelled.
- **Only a few type-specific purl normalizations** (github, bitbucket, pypi)
  are applied. For example, an npm or Maven name that changes case is reported
  as a removal plus an addition.
- **Platform qualifiers are identity.** `arch`, `distro`, `os`, `classifier`
  and so on stay in the key. Moving a Debian package from `distro=debian-12`
  to `distro=debian-13` is reported as a removal plus an addition.
- **Renames are not detected.** A package that changes its name or namespace
  (for example a fork or a moved Maven group) is a removal plus an addition.
- **Fallback identities are only as good as the generator.** Two different
  components without purls that share type, group and name are treated as one
  identity. If they have different versions, they fall into the multi-entry
  rules above and usually end up ambiguous.
- **`metadata.component` and its nested components are not compared.** The
  subject's version is shown in the header instead.
- **Only the fields above are validated.** A document that passes may still
  break other rules of the CycloneDX schema.
