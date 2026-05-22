# Getting Started

This guide is for running MineCLI against a real Minecraft server folder.

## Install From Release

```bash
curl -fsSL https://raw.githubusercontent.com/vincentsaluzzo/mine-cli/main/scripts/install-release.sh | sh
```

Install a specific version:

```bash
MINECLI_VERSION=v0.1.0 sh -c "$(curl -fsSL https://raw.githubusercontent.com/vincentsaluzzo/mine-cli/main/scripts/install-release.sh)"
```

## Install With Cargo

Install the latest code from GitHub:

```bash
cargo install --git https://github.com/vincentsaluzzo/mine-cli minecli
```

After MineCLI is published to crates.io, install it with:

```bash
cargo install minecli
```

## Build From Source

Build a local release binary:

```bash
cargo build --release
```

Install it into `~/.local/bin`:

```bash
./scripts/install.sh
```

Make sure `~/.local/bin` is in your `PATH`, then verify:

```bash
minecli --version
```

## Initialize A Server

Run MineCLI against the server folder:

```bash
minecli --path /srv/minecraft/survival init
```

If auto-detection is not enough, pass explicit values:

```bash
minecli --path /srv/minecraft/survival init --type purpur --minecraft 1.21.5 --name survival
```

MineCLI writes `.minecli/server.toml`, `.minecli/lock.toml`, and `.minecli/history.log` inside the server folder.

## Register Servers

For repeated use, register your server:

```bash
minecli servers add survival /srv/minecraft/survival
minecli --server survival status
```

## First Production Workflow

Before changing files, inspect the server:

```bash
minecli --server survival status
minecli --server survival doctor
minecli --server survival import --dry-run
```

If the import plan looks correct:

```bash
minecli --server survival import
```

Search and install:

```bash
minecli --server survival search bluemap
minecli --server survival search farmers-delight-refabricated
minecli --server survival search farmers-delight-refabricated --minecraft 26.1.2
minecli --server survival --dry-run install bluemap
minecli --server survival install bluemap
```

Update with a dry run first:

```bash
minecli --server survival outdated --changelog
minecli --server survival --dry-run update --all
minecli --server survival update --all
```

Rollback is available for update/remove operations:

```bash
minecli --server survival backups list
minecli --server survival rollback <operation-id>
```

## Shell Completions

Generate completions for your shell:

```bash
minecli completions zsh > ~/.zfunc/_minecli
minecli completions bash > ~/.local/share/bash-completion/completions/minecli
minecli completions fish > ~/.config/fish/completions/minecli.fish
```
