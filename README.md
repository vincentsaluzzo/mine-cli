# MineCLI

MineCLI is a command-line tool for Minecraft server administrators. Its goal is to install, update, list, edit, and remove server-side mods, datapacks, and plugins across multiple Minecraft server types.

The project is intentionally starting with Modrinth as the first package source, so package discovery and version metadata come from an existing ecosystem instead of a custom database.

## Current Status

This repository is in Phase 0: project setup.

The current executable supports:

```bash
cargo run -- --help
cargo run -- --version
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

## License

MineCLI is distributed under the MIT license. See [LICENSE](LICENSE).
