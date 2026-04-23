#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage: build-deb.sh --binary PATH --version VERSION --deb-version VERSION --arch ARCH --output PATH --repo-url URL
EOF
}

binary=""
version=""
deb_version=""
arch=""
output=""
repo_url=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      binary="${2:-}"
      shift
      ;;
    --version)
      version="${2:-}"
      shift
      ;;
    --deb-version)
      deb_version="${2:-}"
      shift
      ;;
    --arch)
      arch="${2:-}"
      shift
      ;;
    --output)
      output="${2:-}"
      shift
      ;;
    --repo-url)
      repo_url="${2:-}"
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
  shift
done

if [[ -z "$binary" || -z "$version" || -z "$deb_version" || -z "$arch" || -z "$output" || -z "$repo_url" ]]; then
  usage
  exit 1
fi

if [[ ! -x "$binary" ]]; then
  echo "Binary not found or not executable: $binary" >&2
  exit 1
fi

case "$arch" in
  amd64 | arm64)
    ;;
  *)
    echo "Unsupported Debian architecture: $arch" >&2
    exit 1
    ;;
esac

pkgroot="$(mktemp -d)"
cleanup() {
  rm -rf "$pkgroot"
}
trap cleanup EXIT

mkdir -p \
  "$pkgroot/DEBIAN" \
  "$pkgroot/usr/bin" \
  "$pkgroot/usr/share/doc/codex"

install -m 0755 "$binary" "$pkgroot/usr/bin/codex"

cat >"$pkgroot/usr/share/doc/codex/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Source: $repo_url
License: Apache-2.0
EOF

installed_size="$(du -sk "$pkgroot/usr" | awk '{print $1}')"

cat >"$pkgroot/DEBIAN/control" <<EOF
Package: codex
Version: $deb_version
Architecture: $arch
Maintainer: Daulet <noreply@github.com>
Installed-Size: $installed_size
Depends: ca-certificates, ripgrep
Section: utils
Priority: optional
Homepage: $repo_url
Description: Codex CLI
 A fork of the OpenAI Codex CLI with persistent tree and side conversation support.
EOF

mkdir -p "$(dirname "$output")"
dpkg-deb --build --root-owner-group "$pkgroot" "$output"

echo "Built $output for version $version ($arch)"
