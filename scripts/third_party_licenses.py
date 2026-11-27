#!/usr/bin/env python3
"""Write THIRD-PARTY-LICENSES.txt for the crates compiled into the binary.

Usage: python3 scripts/third_party_licenses.py OUTPUT_FILE

Reads `cargo metadata --locked` (no network once dependencies are fetched),
follows normal and build dependencies of this package for all targets, and
writes each crate's declared license plus its license files. Identical
license texts are written once, followed by the crates that use them.
"""

import json
import os
import subprocess
import sys

LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE")


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    output = sys.argv[1]

    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]

    seen = set()
    stack = [root]
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            kinds = {k["kind"] for k in dep["dep_kinds"]}
            if kinds & {None, "build"}:
                stack.append(dep["pkg"])
    seen.discard(root)

    crates = sorted((packages[i] for i in seen), key=lambda p: (p["name"], p["version"]))
    texts = {}
    lines = [
        "Third-party licenses for sbom-release-diff",
        "",
        "The sbom-release-diff binary is built from the following Rust crates.",
        "Each crate is listed with its declared license; the license texts",
        "follow, each one once, with the crates that ship it.",
        "",
    ]
    for crate in crates:
        lines.append(f"- {crate['name']} {crate['version']}: {crate['license']}")
        directory = os.path.dirname(crate["manifest_path"])
        names = sorted(
            f for f in os.listdir(directory) if f.upper().startswith(LICENSE_PREFIXES)
        )
        if not names:
            raise SystemExit(f"no license file found for {crate['name']} {crate['version']}")
        for name in names:
            path = os.path.join(directory, name)
            if not os.path.isfile(path):
                continue
            with open(path, encoding="utf-8", errors="replace") as handle:
                text = handle.read().strip().replace("\r\n", "\n")
            texts.setdefault(text, []).append(f"{crate['name']} {crate['version']} ({name})")

    for text, users in sorted(texts.items(), key=lambda item: item[1][0]):
        lines += ["", "=" * 78, "Used by:"]
        lines += [f"  {user}" for user in users]
        lines += ["-" * 78, text]

    with open(output, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
