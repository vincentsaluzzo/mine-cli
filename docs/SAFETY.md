# Safety Model

MineCLI is designed to change only files it owns or explicitly installs.

## Local State

Each server folder gets:

```text
.minecli/
  server.toml
  lock.toml
  history.log
  backups/
  datapacks-disabled/
```

`lock.toml` is the ownership boundary. Remove, update, rollback, and diagnostics use it to avoid deleting unmanaged files.

## Dry Runs

Use `--dry-run` before operations that can change files:

```bash
minecli --server survival --dry-run install bluemap
minecli --server survival --dry-run update --all
minecli --server survival import --dry-run
```

## Backups

Update and remove operations create backups before replacing or deleting tracked files:

```bash
minecli --server survival backups list
minecli --server survival rollback <operation-id>
```

Backups include metadata and the previous files, so rollback can restore the lockfile entry and file path.

## Diagnostics

`doctor` checks:

- missing configured directories
- missing tracked files
- hash mismatches
- stale lockfile entries
- duplicate installed files
- source metadata issues when available

`doctor --fix` only applies safe local fixes: creating missing content directories and removing stale lockfile entries.

## Datapacks

`datapacks disable` and `datapacks enable` move datapack files between the world datapack folder and `.minecli/datapacks-disabled/`. MineCLI does not edit `level.dat`.

## Registry Sources

Modrinth installs verify SHA-512 when available and SHA-1 as a fallback. Downloads go through a cache and are copied into the server after verification.
