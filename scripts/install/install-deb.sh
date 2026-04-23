#!/usr/bin/env sh

set -eu

REPO="${CODEX_RELEASE_REPO:-daulet/codex}"
RELEASE="${CODEX_RELEASE:-latest}"

download_file() {
  url="$1"
  output="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -q -O "$output" "$url"
    return
  fi

  echo "curl or wget is required to install Codex." >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 is required to install Codex." >&2
    exit 1
  fi
}

case "$(uname -s)" in
  Linux)
    ;;
  *)
    echo "install-deb.sh supports Debian/Ubuntu Linux only." >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  x86_64 | amd64)
    deb_arch="amd64"
    ;;
  arm64 | aarch64)
    deb_arch="arm64"
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

require_command sha256sum
require_command apt-get

asset="codex-$deb_arch.deb"

case "$RELEASE" in
  latest)
    base_url="https://github.com/$REPO/releases/latest/download"
    ;;
  v* | codex-v* | rust-v*)
    base_url="https://github.com/$REPO/releases/download/$RELEASE"
    ;;
  *)
    base_url="https://github.com/$REPO/releases/download/v$RELEASE"
    ;;
esac

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

deb_path="$tmp_dir/$asset"
sums_path="$tmp_dir/SHA256SUMS"

echo "Downloading Codex package for $deb_arch from $REPO"
download_file "$base_url/$asset" "$deb_path"
download_file "$base_url/SHA256SUMS" "$sums_path"

expected="$(grep "  $asset\$" "$sums_path" | awk '{print $1}')"
if [ -z "$expected" ]; then
  echo "Could not find checksum for $asset in SHA256SUMS." >&2
  exit 1
fi

actual="$(sha256sum "$deb_path" | awk '{print $1}')"
if [ "$actual" != "$expected" ]; then
  echo "Downloaded Codex package checksum did not match release metadata." >&2
  echo "expected: $expected" >&2
  echo "actual:   $actual" >&2
  exit 1
fi

echo "Installing Codex package"
if [ "$(id -u)" -eq 0 ]; then
  apt-get install -y "$deb_path"
elif command -v sudo >/dev/null 2>&1; then
  sudo apt-get install -y "$deb_path"
else
  echo "sudo is required to install Codex as a non-root user." >&2
  exit 1
fi

echo "Codex installed. Run: codex"
