# MineCLI

MineCLI is a command-line tool for Minecraft server administrators. Its goal is to install, update, list, edit, and remove server-side mods, datapacks, and plugins across multiple Minecraft server types.

The project is intentionally starting with Modrinth as the first package source, so package discovery and version metadata come from an existing ecosystem instead of a custom database.

## Current Status

This repository is in Phase 1: local server MVP.

The current executable supports:

```bash
cargo run -- --help
cargo run -- --version
cargo run -- --path /srv/minecraft/survival init --type fabric --minecraft 1.21.5
cargo run -- --path /srv/minecraft/survival search fabric-api --kind mod
cargo run -- --path /srv/minecraft/survival --dry-run install fabric-api --kind mod
cargo run -- --path /srv/minecraft/survival list
cargo run -- --path /srv/minecraft/survival status
cargo run -- --path /srv/minecraft/survival doctor
```

MineCLI stores per-server state in `.minecli/` inside the server folder:

```text
.minecli/
  server.toml
  lock.toml
  history.log
```

The implementation roadmap is tracked in:

- [PROJECT_PLAN.md](PROJECT_PLAN.md)
- [TASKS.md](TASKS.md)

## Development

Required tooling:

- Rust 1.85 or newer
- Cargo

Common commands:

```bash
make fmt
make lint
make test
```

Equivalent Cargo commands:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Live integration tests are ignored by default because they download real Fabric, Purpur, Forge, and NeoForge server artifacts and install packages from Modrinth:

```bash
cargo test --test live_server_flows -- --ignored --nocapture
```

Use `--nocapture` to see each downloaded artifact, detected server type, install target, lockfile listing, and cleanup check.

## License

MineCLI is distributed under the MIT license. See [LICENSE](LICENSE).
