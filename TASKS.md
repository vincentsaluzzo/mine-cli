# MineCLI Tasks

This file is the implementation checklist. Work through it phase by phase. Keep tasks small enough that each one can be completed and verified independently.

## Phase 0: Project Setup

- [x] Choose the implementation language.
- [x] Initialize the repository.
- [x] Add a license.
- [x] Add a basic `README.md`.
- [x] Add a basic command that prints the CLI version.
- [x] Add formatting and linting commands.
- [x] Add a test command.
- [x] Add CI once the first code exists.

## Phase 1: Local Server MVP

Goal: manage one Minecraft server from inside its folder.

### CLI Foundation

- [x] Add the `minecli` executable entrypoint.
- [x] Add global flags: `--path`, `--config`, `--dry-run`, `--yes`, `--verbose`.
- [x] Add structured error handling.
- [x] Add user-friendly terminal output.
- [x] Add a command dispatch structure that keeps commands isolated.

### Local State

- [x] Define the `.minecli/` directory layout.
- [x] Define `server.toml`.
- [x] Define `lock.toml`.
- [x] Define `history.log`.
- [x] Implement loading local server config.
- [x] Implement writing local server config.
- [x] Implement loading the lockfile.
- [x] Implement writing the lockfile atomically.
- [x] Add tests for config and lockfile serialization.

### Init Command

- [x] Implement `minecli init`.
- [x] Support `minecli init --type <server-type> --minecraft <version>`.
- [x] Create `.minecli/server.toml`.
- [x] Create an empty `.minecli/lock.toml`.
- [x] Detect default paths for `mods`, `plugins`, and datapacks.
- [x] Refuse to overwrite existing MineCLI state unless `--force` is passed.
- [x] Add tests for init behavior.

### Server Model

- [x] Define supported server types.
- [x] Define supported content kinds: `mod`, `plugin`, `datapack`.
- [x] Map server type and content kind to install paths.
- [x] Add basic server type detection from common jar filenames.
- [x] Add validation for server type and content kind compatibility.
- [x] Add tests for install path resolution.

### Modrinth Client

- [x] Implement a Modrinth HTTP client.
- [x] Set a clear User-Agent for all Modrinth requests.
- [x] Implement project search.
- [x] Implement project lookup by slug or ID.
- [x] Implement version listing for a project.
- [x] Implement loader tag fetching.
- [x] Implement project type tag fetching.
- [x] Implement response caching where useful.
- [x] Add tests with mocked Modrinth responses.
- [x] Keep CLI help and user-facing command language package-source agnostic.

### Search Command

- [x] Implement `minecli search <query>`.
- [x] Filter search by Minecraft version from `server.toml`.
- [x] Filter search by server loader or type.
- [x] Filter search by content kind when `--kind` is passed.
- [x] Hide client-only projects by default for server installs.
- [x] Display slug, title, project type, downloads, server support, and summary.
- [x] Add tests for search query construction.

### Version Selection

- [x] Define version selection rules.
- [x] Prefer `release` versions by default.
- [x] Allow `--channel beta`.
- [x] Allow `--channel alpha`.
- [x] Select only versions matching Minecraft version.
- [x] Select only versions matching loader or server type.
- [x] Select the primary file when available.
- [x] Fall back to the first file only when no primary file exists.
- [x] Add tests for version selection.

### Dependency Resolution

- [x] Read dependencies from Modrinth version metadata.
- [x] Install `required` dependencies automatically.
- [x] Show `optional` dependencies without installing by default.
- [x] Warn about `incompatible` dependencies.
- [x] Treat `embedded` dependencies as informational.
- [x] Avoid installing duplicate dependencies.
- [x] Detect dependency cycles.
- [x] Add tests for required dependency resolution.

### Install Planning

- [x] Build an install plan before touching files.
- [x] Include target paths in the plan.
- [x] Include dependency installs in the plan.
- [x] Detect conflicts with existing files.
- [x] Detect when a package is already installed.
- [x] Support `minecli install <project> --dry-run`.
- [x] Print a readable install summary.
- [x] Add tests for install planning.

### Download And Verification

- [x] Define the global download cache path.
- [x] Download files to a temporary path first.
- [x] Verify SHA-512 when available.
- [x] Verify SHA-1 as a fallback.
- [x] Move verified downloads into cache.
- [x] Copy cached files into the server folder.
- [x] Handle interrupted downloads safely.
- [x] Add tests for hash verification.

### Install Command

- [x] Implement `minecli install <project>`.
- [x] Support `--kind mod`.
- [x] Support `--kind plugin`.
- [x] Support `--kind datapack`.
- [x] Support `--version <version-id-or-number>`.
- [x] Support `--channel <release|beta|alpha>`.
- [x] Support `--no-deps`.
- [x] Update `lock.toml` after successful install.
- [x] Record the install in `history.log`.
- [x] Add integration tests for install using mocked downloads.
- [x] Add ignored live integration tests for Fabric, Purpur, Forge, and NeoForge server folders.

### List Command

- [x] Implement `minecli list`.
- [x] Show installed package slug, kind, version, source, and path.
- [x] Support filtering by `--kind`.
- [x] Support machine-readable output with `--json`.
- [x] Add tests for list output.

### Remove Command

- [x] Implement `minecli remove <project>`.
- [x] Resolve packages by slug, project ID, or installed filename.
- [x] Remove only files tracked in `lock.toml`.
- [x] Warn when another installed package depends on the target.
- [x] Support `--remove-orphans` for unused dependencies.
- [x] Support `--dry-run`.
- [x] Update `lock.toml` after successful removal.
- [x] Record the removal in `history.log`.
- [x] Add tests for safe removal.

### Status And Doctor

- [x] Implement `minecli status`.
- [x] Show server name, type, Minecraft version, and package counts.
- [x] Show whether unmanaged files exist in known content folders.
- [x] Implement `minecli doctor`.
- [x] Detect missing target directories.
- [x] Detect lockfile entries whose files are missing.
- [x] Detect installed files whose hashes do not match the lockfile.
- [x] Add tests for status and doctor checks.

## Phase 2: Real Server Administration

Goal: make MineCLI comfortable for admins managing real servers over time.

### Global Config

- [x] Define `~/.config/minecli/config.toml`.
- [x] Define `~/.config/minecli/servers.toml`.
- [x] Implement global config loading.
- [x] Implement global config writing.
- [x] Support platform-correct config paths.
- [x] Add tests for global config behavior.

### Server Registry

- [x] Implement `minecli servers list`.
- [x] Implement `minecli servers add <name> <path>`.
- [x] Implement `minecli servers remove <name>`.
- [x] Implement `minecli servers show <name>`.
- [x] Support `minecli --server <name> <command>`.
- [x] Detect duplicate names.
- [x] Detect missing server paths.
- [x] Add tests for registry commands.

### Update And Outdated

- [x] Implement `minecli outdated`.
- [x] Query latest compatible Modrinth versions for installed packages.
- [x] Compare installed version IDs against latest compatible version IDs.
- [x] Show changelog summaries when requested.
- [x] Implement `minecli update`.
- [x] Support `minecli update <project>`.
- [x] Support `minecli update --all`.
- [x] Support `minecli update --dry-run`.
- [x] Update dependency versions when required.
- [x] Add tests for update planning.

### Backups And Rollback

- [x] Create backups before replacing or deleting installed files.
- [x] Store backups under `.minecli/backups/`.
- [x] Record backup metadata.
- [x] Implement `minecli backups list`.
- [x] Implement `minecli rollback <operation-id>`.
- [x] Validate rollback targets before restoring.
- [x] Add tests for backup creation and rollback.

### Better Detection

- [x] Detect server type from common jar names.
- [x] Detect server type from startup scripts when possible.
- [x] Detect Minecraft version from server jar metadata when possible.
- [x] Detect world folder from `server.properties`.
- [x] Detect datapack path from configured world name.
- [x] Add tests for detection heuristics.
- [x] Document compatibility with TheRemote/Legendary-Minecraft-Purpur-Geyser Docker volume layout.

### Editing

- [x] Implement `minecli edit`.
- [x] Open `.minecli/server.toml` in `$EDITOR`.
- [x] Validate config after editing.
- [x] Refuse invalid edits unless `--force` is passed.
- [x] Add tests for validation logic.

### Diagnostics

- [x] Improve `minecli doctor` with actionable repair suggestions.
- [x] Add `minecli doctor --fix` for safe automatic fixes.
- [x] Detect stale lockfile entries.
- [x] Detect duplicate installed files.
- [x] Detect likely client-only mods installed on a server.
- [x] Detect incompatible package metadata when Modrinth exposes it.
- [x] Add tests for diagnostics.

## Phase 3: Ecosystem Expansion

Goal: support broader workflows beyond direct registry installation.

### Import Existing Servers

- [x] Implement `minecli import`.
- [x] Scan existing `mods/`, `plugins/`, and datapacks.
- [x] Match files to Modrinth versions by hash when possible.
- [x] Add matched files to `lock.toml`.
- [x] Mark unmatched files as unmanaged.
- [x] Support `minecli import --dry-run`.
- [x] Add tests for hash-based import.

### Export And Sync

- [x] Implement `minecli export`.
- [x] Export a portable manifest.
- [x] Support restoring from an exported manifest.
- [x] Support syncing one server's MineCLI manifest into another server.
- [x] Add tests for export and restore.

### Additional Sources

- [x] Support local file installs.
- [x] Support local folder installs.
- [ ] Evaluate Hangar support for Paper ecosystem packages.
- [ ] Evaluate CurseForge support and API constraints.
- [ ] Define a minimal source trait or interface.
- [ ] Add source priority rules.
- [ ] Support source-specific package IDs in the lockfile.
- [ ] Add tests for multi-source resolution.

### Modpack Support

- [ ] Investigate Modrinth modpack metadata.
- [ ] Support installing server-compatible modpacks where possible.
- [ ] Support extracting server-side package lists from modpacks.
- [ ] Detect unsupported client-only modpacks.
- [ ] Add tests for modpack parsing.

### Server Process Hooks

- [ ] Add optional pre-install hooks.
- [ ] Add optional post-install hooks.
- [ ] Add optional stop/start/restart commands in `server.toml`.
- [ ] Support `minecli update --restart`.
- [ ] Ensure hooks are explicit and never guessed.
- [ ] Add tests for hook command planning.

### Log-Based Diagnostics

- [ ] Implement log file discovery.
- [ ] Parse common missing dependency errors.
- [ ] Parse common incompatible version errors.
- [ ] Suggest MineCLI install or update commands from log findings.
- [ ] Keep log diagnostics read-only by default.
- [ ] Add tests using sample logs.

## Phase 4: Polish And Distribution

Goal: make MineCLI easy to install, document, and trust.

### Documentation

- [ ] Write getting started documentation.
- [ ] Document supported server types.
- [ ] Document server folder layout.
- [ ] Document Modrinth behavior.
- [ ] Document safety behavior.
- [ ] Document global server registry usage.
- [ ] Add command examples.

### Packaging

- [ ] Build release binaries.
- [ ] Add shell completions.
- [ ] Add Homebrew packaging if useful.
- [ ] Add install script if useful.
- [ ] Add checksums for releases.

### Quality

- [ ] Add end-to-end tests for main workflows.
- [ ] Add fixtures for Modrinth API responses.
- [ ] Add filesystem sandbox tests.
- [ ] Add snapshot tests for CLI output.
- [ ] Add performance checks for large lockfiles.
- [ ] Add dependency audit tooling.

### Release

- [ ] Define versioning policy.
- [ ] Create initial `CHANGELOG.md`.
- [ ] Tag the first alpha release.
- [ ] Publish binaries.
- [ ] Collect real-server feedback.
- [ ] Adjust the roadmap based on actual admin workflows.
