# Changelog

All notable changes to MineCLI are documented here.

The project follows the versioning policy in [docs/VERSIONING.md](docs/VERSIONING.md).

## 0.1.1 - 2026-05-22

### Changed

- Search now behaves like Modrinth discovery by default instead of silently filtering by the current server.
- Added explicit search filters for `--minecraft`, `--loader`, and `--server-compatible`.
- Search results now render as readable numbered blocks with compact downloads, version summaries, and terminal color when supported.
- Purpur/Paper plugin compatibility now checks Paper, Spigot, and Bukkit Modrinth artifacts where applicable.
- Added `install --force` to intentionally bypass server kind, loader, and Minecraft version compatibility checks while keeping hash verification.

## 0.1.0 - 2026-05-17

### Added

- Local server initialization with `.minecli/server.toml`, `.minecli/lock.toml`, and `history.log`.
- Server detection for common Minecraft server layouts, including Purpur Docker volume layouts.
- Global server registry with `servers list/add/remove/show` and `--server`.
- Modrinth search, install, update, outdated, dependency resolution, and hash verification.
- Source-neutral install support for local files and folders.
- Import existing server files by Modrinth hash matching.
- Export, restore, and sync workflows.
- Backups and rollback for update/remove operations.
- `doctor` diagnostics and `doctor --fix`.
- Datapack listing, enable, and disable workflows.
- Modrinth `.mrpack` inspection and server-side install support.
- Source-specific lockfile metadata for future Hangar and CurseForge support.
- Shell completion generation.

### Safety

- Dry-run support for risky workflows.
- SHA-512/SHA-1 verification for downloaded files.
- Backups before replacing or deleting tracked files.
- Lockfile ownership checks before removing files.

### Known Limitations

- Hangar and CurseForge are evaluated but not implemented as active package sources.
- Modpack installs skip optional server files.
- Datapack enable/disable is filesystem-based and does not edit `level.dat`.
- Homebrew packaging is not available yet.
