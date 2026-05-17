# Packaging

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

Homebrew packaging is deferred until there is a tagged public alpha with binary artifacts and stable checksums.
