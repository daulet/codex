#!/usr/bin/env python3
"""Generate the Homebrew formula for fork release artifacts."""

from __future__ import annotations

import argparse
from pathlib import Path


DARWIN_ARM64 = "codex-aarch64-apple-darwin.tar.gz"
DARWIN_X64 = "codex-x86_64-apple-darwin.tar.gz"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tap-root", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--repo", required=True, help="GitHub repository in owner/name form.")
    parser.add_argument("--sha256sums", type=Path, required=True)
    return parser.parse_args()


def read_sha256s(path: Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if len(parts) != 2:
            continue
        digest, name = parts
        checksums[name] = digest
    return checksums


def require_checksum(checksums: dict[str, str], asset: str) -> str:
    try:
        return checksums[asset]
    except KeyError as exc:
        raise SystemExit(f"Missing checksum for {asset}") from exc


def formula_text(
    *,
    repo: str,
    version: str,
    release_tag: str,
    darwin_arm64_sha256: str,
    darwin_x64_sha256: str,
) -> str:
    base_url = f"https://github.com/{repo}/releases/download/{release_tag}"
    return f'''class Codex < Formula
  desc "Codex CLI fork with persistent tree and side conversations"
  homepage "https://github.com/{repo}"
  license "Apache-2.0"
  version "{version}"

  depends_on "ripgrep"

  on_macos do
    on_intel do
      url "{base_url}/{DARWIN_X64}"
      sha256 "{darwin_x64_sha256}"
    end
    on_arm do
      url "{base_url}/{DARWIN_ARM64}"
      sha256 "{darwin_arm64_sha256}"
    end
  end

  def install
    bin.install "codex"
  end

  test do
    assert_match "codex", shell_output("#{{bin}}/codex --version")
  end
end
'''


def main() -> int:
    args = parse_args()
    checksums = read_sha256s(args.sha256sums)
    formula_dir = args.tap_root / "Formula"
    formula_dir.mkdir(parents=True, exist_ok=True)
    formula_path = formula_dir / "codex.rb"
    formula_path.write_text(
        formula_text(
            repo=args.repo,
            version=args.version,
            release_tag=args.release_tag,
            darwin_arm64_sha256=require_checksum(checksums, DARWIN_ARM64),
            darwin_x64_sha256=require_checksum(checksums, DARWIN_X64),
        ),
        encoding="utf-8",
    )
    print(f"Wrote {formula_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
