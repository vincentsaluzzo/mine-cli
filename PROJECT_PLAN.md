# MineCLI Project Plan

## Goal

MineCLI is a command-line tool for Minecraft server administrators. It helps install, update, list, edit, and remove server-side content across different Minecraft server types:

- mods
- datapacks
- plugins

The first package source should be Modrinth, using its public listings and API metadata instead of maintaining a custom package database.

## Product Direction

MineCLI should work naturally from inside a Minecraft server folder:

```bash
cd /srv/minecraft/survival
minecli install lithium
minecli list
minecli update
```

It should also support an optional global registry for admins managing many servers:

```bash
minecli servers list
minecli --server survival install fabric-api
minecli --server lobby update
```

The server folder should remain the source of truth. Global state should only improve convenience and caching.

## State Model

Use a hybrid state model:

- Each managed server has local state in `.minecli/`.
- The user can optionally register known servers globally.
- Downloads can be cached globally to avoid repeatedly downloading the same files.

This keeps servers portable and easy to back up while still supporting multi-server administration.

### Local Server State

Each server folder should contain:

```text
.minecli/
  server.toml
  lock.toml
  history.log
  backups/
```

`server.toml` describes the server:

```toml
name = "survival"
minecraft_version = "1.21.5"
server_type = "fabric"
world = "world"

[paths]
mods = "mods"
plugins = "plugins"
datapacks = "world/datapacks"
```

`lock.toml` records exactly what MineCLI installed:

```toml
[[packages]]
source = "modrinth"
project_id = "P7dR8mSH"
slug = "fabric-api"
kind = "mod"
loader = "fabric"
version_id = "abc123"
filename = "fabric-api.jar"
sha512 = "..."
installed_path = "mods/fabric-api.jar"
dependencies = []
```

The lockfile is critical. MineCLI should only remove or update files it owns unless the user explicitly forces the action.

### Global User State

Global state should be optional:

```text
~/.config/minecli/config.toml
~/.config/minecli/servers.toml
~/.cache/minecli/downloads/
```

`config.toml` stores user preferences. `servers.toml` maps friendly server names to folders. The cache stores downloaded files by hash.

## Modrinth Integration

Modrinth should be treated as the first package registry.

MineCLI should use Modrinth data for:

- searching projects
- filtering by Minecraft version
- filtering by loader or server type
- filtering by project type
- checking server-side support
- listing versions
- selecting compatible releases
- resolving required dependencies
- downloading primary files
- verifying file hashes

Important Modrinth concepts:

- Search facets include project type, categories/loaders, Minecraft versions, and client/server side support.
- Project versions can be filtered by loaders and Minecraft versions.
- Version dependencies can be required, optional, incompatible, or embedded.
- Files include URLs, filenames, primary flags, sizes, and hashes.
- Loader tags should be queried and cached instead of hardcoded.

Do not build a permanent package catalog. Query Modrinth as needed and cache responses conservatively.

## Server Types And Install Targets

Initial target rules:

| Server type | Content type | Install folder |
| --- | --- | --- |
| Fabric | mod | `mods/` |
| Quilt | mod | `mods/` |
| Forge | mod | `mods/` |
| NeoForge | mod | `mods/` |
| Paper | plugin | `plugins/` |
| Purpur | plugin | `plugins/` |
| Spigot | plugin | `plugins/` |
| Bukkit | plugin | `plugins/` |
| Folia | plugin | `plugins/` |
| Sponge | plugin | `plugins/` |
| Velocity | plugin | `plugins/` |
| Waterfall | plugin | `plugins/` |
| BungeeCord | plugin | `plugins/` |
| Any Java server | datapack | `<world>/datapacks/` |

Server type detection can start simple and improve later:

```text
fabric-server-launch.jar -> fabric
forge-*.jar              -> forge
neoforge-*.jar           -> neoforge
paper-*.jar              -> paper
purpur-*.jar             -> purpur
spigot-*.jar             -> spigot
server.properties only   -> vanilla or unknown
```

When detection is uncertain, MineCLI should ask the user during `init`, or require explicit flags in non-interactive mode.

## CLI Design

Initial commands:

```bash
minecli init
minecli status
minecli search <query>
minecli install <project>
minecli remove <project>
minecli list
minecli update
minecli outdated
minecli info <project>
minecli edit
minecli doctor
```

Multi-server commands:

```bash
minecli servers list
minecli servers add <name> <path>
minecli servers remove <name>
minecli --server <name> <command>
minecli --path <path> <command>
```

Examples:

```bash
minecli init --type fabric --minecraft 1.21.5
minecli search "voice chat" --kind mod
minecli install simple-voice-chat
minecli install lithium --version latest
minecli install bluemap --kind plugin
minecli remove bluemap
minecli update --dry-run
minecli doctor
```

## Install Flow

Installing a package should follow a deterministic plan:

1. Resolve the target server folder.
2. Load `.minecli/server.toml`.
3. Detect or validate server type.
4. Search Modrinth by slug or query.
5. Filter by Minecraft version, loader, project type, and server-side support.
6. Select a compatible version.
7. Resolve required dependencies recursively.
8. Build an install plan.
9. Show the plan when dependency changes are involved.
10. Download files into cache.
11. Verify hashes.
12. Copy or link files into the right server folder.
13. Update `.minecli/lock.toml`.
14. Record the operation in `.minecli/history.log`.

Default behavior should prefer stable release versions. Users can opt into beta or alpha releases.

## Remove Flow

Removing a package should:

1. Resolve the package from the lockfile.
2. Check whether other installed packages depend on it.
3. Show impacted dependencies.
4. Remove only files owned by MineCLI.
5. Update the lockfile.
6. Record the operation in history.

MineCLI should not remove manually installed files unless the user passes an explicit force flag.

## Update Flow

Updating should:

1. Read the lockfile.
2. Query Modrinth for each package's latest compatible version.
3. Compare current version IDs with available version IDs.
4. Build an update plan.
5. Include dependency changes.
6. Backup replaced files.
7. Download and verify replacements.
8. Update the lockfile.

The first implementation should support `--dry-run` before modifying files.

## Safety Principles

MineCLI should be conservative by default:

- Never delete unknown files.
- Verify downloaded hashes.
- Keep backups before replacing files.
- Support dry-run mode for installs, removals, and updates.
- Avoid changing server config files unless the user explicitly edits them.
- Prefer clear errors over guessing when server type or compatibility is ambiguous.

## Suggested Technology

Rust is the preferred implementation language for this project:

- single static-ish binary distribution
- strong CLI ecosystem
- good TOML and JSON support
- safe filesystem handling
- good cross-platform support

Suggested Rust crates:

- `clap` for CLI parsing
- `serde` for serialization
- `toml` for config files
- `reqwest` for HTTP
- `tokio` for async runtime
- `sha2` for hash verification
- `directories` for platform config/cache paths
- `thiserror` or `anyhow` for error handling
- `tracing` for structured logs

## Architecture

Recommended module layout:

```text
src/
  main.rs
  cli/
    mod.rs
    commands/
  core/
    server.rs
    manifest.rs
    lockfile.rs
    planner.rs
    resolver.rs
    installer.rs
  sources/
    mod.rs
    modrinth.rs
  fs/
    paths.rs
    download_cache.rs
    backups.rs
  config/
    global.rs
  output/
    table.rs
    prompts.rs
```

Keep the package source abstraction small at first. Avoid designing a full package manager framework before the Modrinth implementation proves the shape.

## Roadmap Summary

Phase 1 should create a practical single-server MVP:

- initialize `.minecli`
- search Modrinth
- install Fabric mods, Paper plugins, and datapacks
- list installed packages
- remove owned packages
- resolve required dependencies
- verify hashes
- support dry-run mode

Phase 2 should make it comfortable for real server administration:

- global server registry
- update and outdated commands
- improved server detection
- backup and rollback
- better diagnostics

Phase 3 should broaden the ecosystem:

- additional sources such as Hangar or CurseForge if feasible
- import existing server contents
- export/import manifests
- modpack support
- server process hooks
- log-based compatibility diagnostics

