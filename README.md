# MineCLI

MineCLI is a command-line tool for Minecraft server administrators. Its goal is to install, update, list, edit, and remove server-side mods, datapacks, and plugins across multiple Minecraft server types.

The CLI is designed to be package-source agnostic. Modrinth is the first implemented source for discovery and version metadata, but commands should stay generic enough to support local files, folders, and other registries later.

## Current Status

This repository is in Phase 2: real server administration.

The current executable supports:

```bash
cargo run -- --help
cargo run -- --version
cargo run -- --path /srv/minecraft/survival init --type fabric --minecraft 1.21.5
cargo run -- --path /srv/minecraft/survival init
cargo run -- --path /srv/minecraft/survival search fabric-api --kind mod
cargo run -- --path /srv/minecraft/survival --dry-run install fabric-api --kind mod
cargo run -- --path /srv/minecraft/survival install --file ./mods/example.jar --kind mod
cargo run -- --path /srv/minecraft/survival install --folder ./mods-to-install --kind mod
cargo run -- --path /srv/minecraft/survival list
cargo run -- --path /srv/minecraft/survival outdated
cargo run -- --path /srv/minecraft/survival --dry-run update --all
cargo run -- --path /srv/minecraft/survival update fabric-api
cargo run -- --path /srv/minecraft/survival backups list
cargo run -- --path /srv/minecraft/survival rollback <operation-id>
cargo run -- --path /srv/minecraft/survival edit
cargo run -- --path /srv/minecraft/survival status
cargo run -- --path /srv/minecraft/survival doctor --fix
cargo run -- servers add survival /srv/minecraft/survival
cargo run -- --server survival status
```

MineCLI stores per-server state in `.minecli/` inside the server folder:

```text
.minecli/
  server.toml
  lock.toml
  history.log
  backups/
```

## Compatible Server Layouts

MineCLI can initialize directly from standard Minecraft server folders and from the Docker volume layout used by [TheRemote/Legendary-Minecraft-Purpur-Geyser](https://github.com/TheRemote/Legendary-Minecraft-Purpur-Geyser).

For that image, MineCLI detects:

- `purpur.jar` and `purpur.yml` as a Purpur server
- `version_history.json`, `versions/`, and `cache/` as version hints
- `server.properties` `level-name` for the world/datapack path
- `plugins/` as the plugin target

Example:

```bash
cargo run -- --path /path/to/docker/volume/_data init --name survival
```

The implementation roadmap is tracked in:

- [PROJECT_PLAN.md](PROJECT_PLAN.md)
- [TASKS.md](TASKS.md)

## Server Registry

MineCLI can remember server folders globally:

```bash
cargo run -- servers add survival /srv/minecraft/survival
cargo run -- servers list
cargo run -- servers show survival
cargo run -- --server survival status
cargo run -- servers remove survival
```

By default the registry is stored in the platform config directory as `servers.toml`. Use `--config <path>` or `MINECLI_CONFIG_DIR` to override the config directory.

## Local Sources

Registry installs are not the only supported source. You can also install local files and folders:

```bash
cargo run -- --server survival install --file ./voicechat.jar --kind plugin
cargo run -- --server survival install --folder ./mods-to-install --kind mod
cargo run -- --server survival install --file ./datapack.zip --kind datapack
```

Local installs are copied into the server's configured `mods`, `plugins`, or datapacks directory and tracked in `.minecli/lock.toml` with SHA-512/SHA-1 hashes.

## Updates

Registry-backed installs can be checked and updated against the latest compatible release for the server's Minecraft version and loader:

```bash
cargo run -- --server survival outdated
cargo run -- --server survival outdated --changelog
cargo run -- --server survival --dry-run update --all
cargo run -- --server survival update fabric-api
```

Local file and folder installs are tracked in the lockfile but intentionally skipped by update commands.

## Backups And Rollback

Update and remove operations create file backups before replacing or deleting tracked files:

```bash
cargo run -- --server survival backups list
cargo run -- --server survival rollback <operation-id>
```

Backups are stored under `.minecli/backups/` with metadata that lets rollback restore both files and lockfile entries.

## Editing And Diagnostics

```bash
cargo run -- --server survival edit
cargo run -- --server survival doctor
cargo run -- --server survival doctor --fix
```

`edit` opens `.minecli/server.toml` in `$VISUAL` or `$EDITOR` and validates the result. `doctor --fix` applies safe local fixes such as creating missing content directories and removing stale lockfile entries.

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

Live integration tests are ignored by default because they download real Fabric, Purpur, Forge, and NeoForge server artifacts and install packages from external sources:

```bash
cargo test --test live_server_flows -- --ignored --nocapture
```

Use `--nocapture` to see each downloaded artifact, detected server type, install target, lockfile listing, and cleanup check.

## License

MineCLI is distributed under the MIT license. See [LICENSE](LICENSE).
