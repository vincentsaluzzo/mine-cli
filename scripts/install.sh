#!/usr/bin/env sh
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="$PREFIX/bin"

cargo build --release
mkdir -p "$BINDIR"
cp target/release/minecli "$BINDIR/minecli"

echo "Installed minecli to $BINDIR/minecli"
echo "Ensure $BINDIR is in your PATH."
