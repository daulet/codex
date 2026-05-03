#!/usr/bin/env python3
"""Update the codex-rs workspace package version."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


VERSION_RE = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
VERSION_LINE_RE = re.compile(r'^(\s*version\s*=\s*)"[^"]*"(.*)$')


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("codex-rs/Cargo.toml"),
        help="Path to the codex-rs workspace Cargo.toml.",
    )
    return parser.parse_args()


def replace_workspace_version(text: str, version: str) -> str:
    lines = text.splitlines(keepends=True)
    in_workspace_package = False
    replaced = False
    updated: list[str] = []

    for line in lines:
        body = line[:-1] if line.endswith("\n") else line
        newline = "\n" if line.endswith("\n") else ""
        stripped = body.strip()

        if stripped.startswith("[") and stripped.endswith("]"):
            in_workspace_package = stripped == "[workspace.package]"

        if in_workspace_package:
            match = VERSION_LINE_RE.match(body)
            if match:
                line = f'{match.group(1)}"{version}"{match.group(2)}{newline}'
                replaced = True

        updated.append(line)

    if not replaced:
        raise SystemExit("Could not find [workspace.package] version in manifest")

    return "".join(updated)


def main() -> int:
    args = parse_args()
    if not VERSION_RE.match(args.version):
        raise SystemExit(f"Version must be stable SemVer x.y.z: {args.version}")

    text = args.manifest.read_text(encoding="utf-8")
    args.manifest.write_text(
        replace_workspace_version(text, args.version),
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
