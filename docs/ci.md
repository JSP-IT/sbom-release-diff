# CI examples

sbom-release-diff takes two files and writes a report to standard output.
How you produce the SBOMs (Syft, cdxgen, `cargo cyclonedx`, a CycloneDX
Maven or Gradle plugin, and so on) and where you keep the previous release's
SBOM is up to your pipeline.

Exit codes: `0` passed, `1` a `--fail-on` policy was violated (the report is
still written), `2` usage or input error (no report).

## Choosing policies

| Goal | Flags |
| --- | --- |
| Report only, never fail | *(none)* |
| A human must approve anything new or gone | `--fail-on added,removed` |
| License review gate | `--fail-on license-changed,ambiguous` |
| Lock file style: any change needs approval | `--fail-on any-change` |
| Also fail on data-quality warnings | add `warning` |

`ambiguous` deserves a place in most gates. It means the tool could not tell
whether two entries are the same component, so a change may be hiding there.

## GitHub Actions

Download a pinned release, verify its checksum, compare, and put the Markdown
report in the job summary:

```yaml
jobs:
  sbom-diff:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    env:
      SBOM_DIFF_VERSION: v0.1.0
      # Copy this value from checksums.txt of the release you pin.
      SBOM_DIFF_SHA256: <sha256 of sbom-release-diff_v0.1.0_x86_64-unknown-linux-musl.tar.gz>
    steps:
      - uses: actions/checkout@v7
        with:
          persist-credentials: false

      # ... produce previous.cdx.json and current.cdx.json here ...

      - name: Install sbom-release-diff
        run: |
          set -eu
          archive="sbom-release-diff_${SBOM_DIFF_VERSION}_x86_64-unknown-linux-musl.tar.gz"
          curl -fsSLO "https://github.com/JSP-IT/sbom-release-diff/releases/download/${SBOM_DIFF_VERSION}/${archive}"
          echo "${SBOM_DIFF_SHA256}  ${archive}" | sha256sum --check --strict
          tar -xzf "$archive"
          echo "$PWD/${archive%.tar.gz}" >> "$GITHUB_PATH"

      - name: Compare SBOMs
        run: |
          set -o pipefail
          sbom-release-diff previous.cdx.json current.cdx.json \
            --format markdown \
            --fail-on removed,license-changed,ambiguous \
            | tee -a "$GITHUB_STEP_SUMMARY"
```

To post the report as a pull request comment instead, write it to a file
(`> sbom-diff.md`) and post it with a step you trust. The Markdown is safe to
render: every SBOM value is inside a code span, and pipes are escaped.

## GitLab CI

```yaml
sbom-diff:
  image: alpine:3
  variables:
    SBOM_DIFF_VERSION: v0.1.0
    SBOM_DIFF_SHA256: "<sha256 from checksums.txt>"
  script:
    - apk add --no-cache curl
    - archive="sbom-release-diff_${SBOM_DIFF_VERSION}_x86_64-unknown-linux-musl.tar.gz"
    - curl -fsSLO "https://github.com/JSP-IT/sbom-release-diff/releases/download/${SBOM_DIFF_VERSION}/${archive}"
    - echo "${SBOM_DIFF_SHA256}  ${archive}" | sha256sum -c -
    - tar -xzf "$archive" && export PATH="$PWD/${archive%.tar.gz}:$PATH"
    - sbom-release-diff previous.cdx.json current.cdx.json --format json > sbom-diff.json || status=$?
    - sbom-release-diff previous.cdx.json current.cdx.json
    - exit "${status:-0}"
  artifacts:
    when: always
    paths: [sbom-diff.json]
```

## Using the JSON report

The JSON report is versioned. Check `report_version` before relying on
fields:

```sh
sbom-release-diff old.json new.json --format json > diff.json

jq -e '.report_format == "sbom-release-diff-report" and .report_version == 1' diff.json

# Components that changed declared licenses
jq -r '.license_changed[] | "\(.identity) \(.old_licenses | join(",")) -> \(.new_licenses | join(","))"' diff.json

# Did the policy pass?
jq -e '.policy.passed' diff.json
```

The schema is [schema/report-v1.schema.json](../schema/report-v1.schema.json).

## Things to avoid

- Do not pass SBOMs from untrusted forks to steps that hold secrets. The tool
  itself makes no network calls and never prints purl location qualifiers,
  but your pipeline around it might.
- Use `--max-input-bytes` if your SBOMs are much smaller than the 64 MiB
  default, so an unexpectedly large input fails fast.
