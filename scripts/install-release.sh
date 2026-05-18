#!/usr/bin/env sh
set -eu

REPO="${MINECLI_REPO:-vincentsaluzzo/mine-cli}"
VERSION="${MINECLI_VERSION:-latest}"
PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="$PREFIX/bin"

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Linux:x86_64)
    target="x86_64-unknown-linux-gnu"
    ;;
  Darwin:x86_64)
    echo "No prebuilt MineCLI release is available for macOS Intel yet." >&2
    echo "Install from source with ./scripts/install.sh instead." >&2
    exit 1
    ;;
  Darwin:arm64)
    target="aarch64-apple-darwin"
    ;;
  *)
    echo "Unsupported platform: $os $arch" >&2
    exit 1
    ;;
esac

if [ "$VERSION" = "latest" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$VERSION" ]; then
    echo "Could not resolve latest MineCLI release" >&2
    exit 1
  fi
fi

url="https://github.com/$REPO/releases/download/$VERSION"
version_without_v="${VERSION#v}"
package="minecli-v${version_without_v}-${target}.tar.gz"

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

echo "Downloading $package"
curl -fsSL "$url/$package" -o "$tmpdir/$package"

if command -v shasum >/dev/null 2>&1; then
  curl -fsSL "$url/$package.sha256" -o "$tmpdir/$package.sha256"
  expected="$(awk '{print $1}' "$tmpdir/$package.sha256")"
  actual="$(shasum -a 256 "$tmpdir/$package" | awk '{print $1}')"
  if [ "$expected" != "$actual" ]; then
    echo "Checksum mismatch for $package" >&2
    exit 1
  fi
fi

tar -xzf "$tmpdir/$package" -C "$tmpdir"
mkdir -p "$BINDIR"
find "$tmpdir" -type f -name minecli -exec cp {} "$BINDIR/minecli" \;
chmod +x "$BINDIR/minecli"

echo "Installed minecli to $BINDIR/minecli"
echo "Ensure $BINDIR is in your PATH."
