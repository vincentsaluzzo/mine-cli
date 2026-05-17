# Packaging

## Install From GitHub Releases

```bash
curl -fsSL https://raw.githubusercontent.com/vincentsaluzzo/mine-cli/main/scripts/install-release.sh | sh
```

Install a specific version:

```bash
MINECLI_VERSION=v0.1.0 sh -c "$(curl -fsSL https://raw.githubusercontent.com/vincentsaluzzo/mine-cli/main/scripts/install-release.sh)"
```

The installer downloads the matching binary for:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

It verifies the release checksum when `shasum` is available.

## Local Release Build

```bash
make release
```

The release binary is written to:

```text
target/release/minecli
```

Generate a checksum:

```bash
make checksums
```

The checksum file is written to:

```text
target/release/minecli.sha256
```

## Install Script

Build from source and install locally:

```bash
./scripts/install.sh
```

By default the script installs to `~/.local/bin`. Override with:

```bash
PREFIX=/usr/local ./scripts/install.sh
```

## Shell Completions

```bash
minecli completions zsh > ~/.zfunc/_minecli
minecli completions bash > ~/.local/share/bash-completion/completions/minecli
minecli completions fish > ~/.config/fish/completions/minecli.fish
```

## Homebrew

Homebrew packaging is deferred until the alpha release has been tested on real servers.
