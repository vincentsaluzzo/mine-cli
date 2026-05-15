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

- [ ] Add the `minecli` executable entrypoint.
- [ ] Add global flags: `--path`, `--config`, `--dry-run`, `--yes`, `--verbose`.
- [ ] Add structured error handling.
- [ ] Add user-friendly terminal output.
- [ ] Add a command dispatch structure that keeps commands isolated.

### Local State

- [ ] Define the `.minecli/` directory layout.
- [ ] Define `server.toml`.
- [ ] Define `lock.toml`.
- [ ] Define `history.log`.
- [ ] Implement loading local server config.
- [ ] Implement writing local server config.
- [ ] Implement loading the lockfile.
- [ ] Implement writing the lockfile atomically.
- [ ] Add tests for config and lockfile serialization.

### Init Command

- [ ] Implement `minecli init`.
- [ ] Support `minecli init --type <server-type> --minecraft <version>`.
- [ ] Create `.minecli/server.toml`.
- [ ] Create an empty `.minecli/lock.toml`.
- [ ] Detect default paths for `mods`, `plugins`, and datapacks.
- [ ] Refuse to overwrite existing MineCLI state unless `--force` is passed.
- [ ] Add tests for init behavior.

### Server Model

- [ ] Define supported server types.
- [ ] Define supported content kinds: `mod`, `plugin`, `datapack`.
- [ ] Map server type and content kind to install paths.
- [ ] Add basic server type detection from common jar filenames.
- [ ] Add validation for server type and content kind compatibility.
- [ ] Add tests for install path resolution.

### Modrinth Client

- [ ] Implement a Modrinth HTTP client.
- [ ] Set a clear User-Agent for all Modrinth requests.
- [ ] Implement project search.
- [ ] Implement project lookup by slug or ID.
- [ ] Implement version listing for a project.
- [ ] Implement loader tag fetching.
- [ ] Implement project type tag fetching.
- [ ] Implement response caching where useful.
- [ ] Add tests with mocked Modrinth responses.

### Search Command

- [ ] Implement `minecli search <query>`.
- [ ] Filter search by Minecraft version from `server.toml`.
- [ ] Filter search by server loader or type.
- [ ] Filter search by content kind when `--kind` is passed.
- [ ] Hide client-only projects by default for server installs.
- [ ] Display slug, title, project type, downloads, server support, and summary.
- [ ] Add tests for search query construction.

### Version Selection

- [ ] Define version selection rules.
- [ ] Prefer `release` versions by default.
- [ ] Allow `--channel beta`.
- [ ] Allow `--channel alpha`.
- [ ] Select only versions matching Minecraft version.
- [ ] Select only versions matching loader or server type.
- [ ] Select the primary file when available.
- [ ] Fall back to the first file only when no primary file exists.
- [ ] Add tests for version selection.

### Dependency Resolution

- [ ] Read dependencies from Modrinth version metadata.
- [ ] Install `required` dependencies automatically.
- [ ] Show `optional` dependencies without installing by default.
- [ ] Warn about `incompatible` dependencies.
- [ ] Treat `embedded` dependencies as informational.
- [ ] Avoid installing duplicate dependencies.
- [ ] Detect dependency cycles.
- [ ] Add tests for required dependency resolution.

### Install Planning

- [ ] Build an install plan before touching files.
- [ ] Include target paths in the plan.
- [ ] Include dependency installs in the plan.
- [ ] Detect conflicts with existing files.
- [ ] Detect when a package is already installed.
- [ ] Support `minecli install <project> --dry-run`.
- [ ] Print a readable install summary.
- [ ] Add tests for install planning.

### Download And Verification

- [ ] Define the global download cache path.
- [ ] Download files to a temporary path first.
- [ ] Verify SHA-512 when available.
- [ ] Verify SHA-1 as a fallback.
- [ ] Move verified downloads into cache.
- [ ] Copy cached files into the server folder.
- [ ] Handle interrupted downloads safely.
- [ ] Add tests for hash verification.

### Install Command

- [ ] Implement `minecli install <project>`.
- [ ] Support `--kind mod`.
- [ ] Support `--kind plugin`.
- [ ] Support `--kind datapack`.
- [ ] Support `--version <version-id-or-number>`.
- [ ] Support `--channel <release|beta|alpha>`.
- [ ] Support `--no-deps`.
- [ ] Update `lock.toml` after successful install.
- [ ] Record the install in `history.log`.
- [ ] Add integration tests for install using mocked downloads.

### List Command

- [ ] Implement `minecli list`.
- [ ] Show installed package slug, kind, version, source, and path.
- [ ] Support filtering by `--kind`.
- [ ] Support machine-readable output with `--json`.
- [ ] Add tests for list output.

### Remove Command

- [ ] Implement `minecli remove <project>`.
- [ ] Resolve packages by slug, project ID, or installed filename.
- [ ] Remove only files tracked in `lock.toml`.
- [ ] Warn when another installed package depends on the target.
- [ ] Support `--remove-orphans` for unused dependencies.
- [ ] Support `--dry-run`.
- [ ] Update `lock.toml` after successful removal.
- [ ] Record the removal in `history.log`.
- [ ] Add tests for safe removal.

### Status And Doctor

- [ ] Implement `minecli status`.
- [ ] Show server name, type, Minecraft version, and package counts.
- [ ] Show whether unmanaged files exist in known content folders.
- [ ] Implement `minecli doctor`.
- [ ] Detect missing target directories.
- [ ] Detect lockfile entries whose files are missing.
- [ ] Detect installed files whose hashes do not match the lockfile.
- [ ] Add tests for status and doctor checks.

## Phase 2: Real Server Administration

Goal: make MineCLI comfortable for admins managing real servers over time.

### Global Config

- [ ] Define `~/.config/minecli/config.toml`.
- [ ] Define `~/.config/minecli/servers.toml`.
- [ ] Implement global config loading.
- [ ] Implement global config writing.
- [ ] Support platform-correct config paths.
- [ ] Add tests for global config behavior.

### Server Registry

- [ ] Implement `minecli servers list`.
- [ ] Implement `minecli servers add <name> <path>`.
- [ ] Implement `minecli servers remove <name>`.
- [ ] Implement `minecli servers show <name>`.
- [ ] Support `minecli --server <name> <command>`.
- [ ] Detect duplicate names.
- [ ] Detect missing server paths.
- [ ] Add tests for registry commands.

### Update And Outdated

- [ ] Implement `minecli outdated`.
- [ ] Query latest compatible Modrinth versions for installed packages.
- [ ] Compare installed version IDs against latest compatible version IDs.
- [ ] Show changelog summaries when requested.
- [ ] Implement `minecli update`.
- [ ] Support `minecli update <project>`.
- [ ] Support `minecli update --all`.
- [ ] Support `minecli update --dry-run`.
- [ ] Update dependency versions when required.
- [ ] Add tests for update planning.

### Backups And Rollback

- [ ] Create backups before replacing or deleting installed files.
- [ ] Store backups under `.minecli/backups/`.
- [ ] Record backup metadata.
- [ ] Implement `minecli backups list`.
- [ ] Implement `minecli rollback <operation-id>`.
- [ ] Validate rollback targets before restoring.
- [ ] Add tests for backup creation and rollback.

### Better Detection

- [ ] Detect server type from common jar names.
- [ ] Detect server type from startup scripts when possible.
- [ ] Detect Minecraft version from server jar metadata when possible.
- [ ] Detect world folder from `server.properties`.
- [ ] Detect datapack path from configured world name.
- [ ] Add tests for detection heuristics.

### Editing

- [ ] Implement `minecli edit`.
- [ ] Open `.minecli/server.toml` in `$EDITOR`.
- [ ] Validate config after editing.
- [ ] Refuse invalid edits unless `--force` is passed.
- [ ] Add tests for validation logic.

### Diagnostics

- [ ] Improve `minecli doctor` with actionable repair suggestions.
- [ ] Add `minecli doctor --fix` for safe automatic fixes.
- [ ] Detect stale lockfile entries.
- [ ] Detect duplicate installed files.
- [ ] Detect likely client-only mods installed on a server.
- [ ] Detect incompatible package metadata when Modrinth exposes it.
- [ ] Add tests for diagnostics.

## Phase 3: Ecosystem Expansion

Goal: support broader workflows beyond direct Modrinth installation.

### Import Existing Servers

- [ ] Implement `minecli import`.
- [ ] Scan existing `mods/`, `plugins/`, and datapacks.
- [ ] Match files to Modrinth versions by hash when possible.
- [ ] Add matched files to `lock.toml`.
- [ ] Mark unmatched files as unmanaged.
- [ ] Support `minecli import --dry-run`.
- [ ] Add tests for hash-based import.

### Export And Sync

- [ ] Implement `minecli export`.
- [ ] Export a portable manifest.
- [ ] Support restoring from an exported manifest.
- [ ] Support syncing one server's MineCLI manifest into another server.
- [ ] Add tests for export and restore.

### Additional Sources

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
