# Versioning Policy

MineCLI uses semantic versioning after `1.0.0`.

Before `1.0.0`, the project uses `0.y.z` versions:

- `0.y.0`: new user-visible features or workflow changes
- `0.y.z`: bug fixes, documentation, tests, and compatibility improvements

Breaking CLI or lockfile changes are allowed before `1.0.0`, but they must be called out in `CHANGELOG.md`.

## Release Readiness

A release should have:

- passing `make check`
- a release binary built with `make release`
- a checksum from `make checksums`
- updated `CHANGELOG.md`
- a git tag matching the package version, such as `v0.1.0`

## Current Status

`0.1.0` is the first alpha line. It is suitable for real-server testing with backups and dry runs, but it should still be treated as pre-1.0 software.
