#!/usr/bin/env python3
"""Plan the next fork release from release-please-style commit messages."""

from __future__ import annotations

import argparse
import re
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path


SEMVER_RE = re.compile(r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)$")
TAG_PREFIX = "rust-v"
TAG_RE = re.compile(
    r"^(?:rust-v|v)(?P<version>(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*))$"
)
CONVENTIONAL_RE = re.compile(
    r"^(?P<type>[A-Za-z][A-Za-z0-9-]*)(?:\([^)]+\))?(?P<breaking>!)?: .+$"
)
BREAKING_FOOTER_RE = re.compile(r"^BREAKING[ -]CHANGE(?::|!)", re.MULTILINE)


@dataclass(frozen=True, order=True)
class Version:
    major: int
    minor: int
    patch: int

    @classmethod
    def parse(cls, value: str) -> "Version":
        match = SEMVER_RE.match(value)
        if not match:
            raise ValueError(f"not a stable SemVer version: {value}")
        return cls(
            major=int(match.group("major")),
            minor=int(match.group("minor")),
            patch=int(match.group("patch")),
        )

    def bump(self, level: str) -> "Version":
        if level == "major":
            return Version(self.major + 1, 0, 0)
        if level == "minor":
            return Version(self.major, self.minor + 1, 0)
        if level == "patch":
            return Version(self.major, self.minor, self.patch + 1)
        raise ValueError(f"unknown bump level: {level}")

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


@dataclass(frozen=True)
class ReleaseTag:
    version: Version
    tag: str


def git(args: list[str]) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def cargo_version(manifest_path: Path) -> Version:
    with manifest_path.open("rb") as manifest:
        data = tomllib.load(manifest)
    version = data["workspace"]["package"]["version"]
    return Version.parse(version)


def release_tags() -> list[ReleaseTag]:
    tags = (
        git(["tag", "--list", "v[0-9]*.[0-9]*.[0-9]*"]).splitlines()
        + git(["tag", "--list", "rust-v[0-9]*.[0-9]*.[0-9]*"]).splitlines()
    )
    release_tags: list[ReleaseTag] = []
    for tag in tags:
        match = TAG_RE.match(tag)
        if match:
            release_tags.append(ReleaseTag(Version.parse(match.group("version")), tag))
    return release_tags


def commits_since(tag: str) -> list[str]:
    raw = subprocess.check_output(
        ["git", "log", "--format=%B%x00", f"{tag}..HEAD"],
        text=False,
    )
    return [
        message.decode("utf-8").strip()
        for message in raw.split(b"\x00")
        if message.strip()
    ]


def commit_bump(message: str) -> str:
    first_line = message.splitlines()[0] if message.splitlines() else ""
    conventional = CONVENTIONAL_RE.match(first_line)

    if conventional and conventional.group("breaking"):
        return "major"
    if BREAKING_FOOTER_RE.search(message):
        return "major"
    if conventional and conventional.group("type").lower() == "feat":
        return "minor"

    # Fork releases are intentionally created for every new commit. Commits
    # that are conventional but not release-please-feat, plus non-conventional
    # commits, fall back to a patch release.
    return "patch"


def highest_bump(messages: list[str]) -> str:
    levels = {"patch": 0, "minor": 1, "major": 2}
    bump = "patch"
    for message in messages:
        candidate = commit_bump(message)
        if levels[candidate] > levels[bump]:
            bump = candidate
    return bump


def print_output(**values: str) -> None:
    for key, value in values.items():
        print(f"{key}={value}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("codex-rs/Cargo.toml"),
        help="Path to the codex-rs workspace Cargo.toml.",
    )
    args = parser.parse_args()

    current_version = cargo_version(args.manifest)
    tags = release_tags()
    latest = max(tags, key=lambda release_tag: release_tag.version, default=None)

    if latest is None or latest.version < current_version:
        print_output(
            should_release="true",
            version=str(current_version),
            release_tag=f"{TAG_PREFIX}{current_version}",
            previous_tag=latest.tag if latest else "",
            bump="patch",
        )
        return 0

    messages = commits_since(latest.tag)
    if not messages:
        print_output(
            should_release="false",
            version=str(latest.version),
            release_tag=latest.tag,
            previous_tag=latest.tag,
            bump="none",
        )
        return 0

    bump = highest_bump(messages)
    next_version = latest.version.bump(bump)
    print_output(
        should_release="true",
        version=str(next_version),
        release_tag=f"{TAG_PREFIX}{next_version}",
        previous_tag=latest.tag,
        bump=bump,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
