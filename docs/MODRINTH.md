# Modrinth Behavior

Modrinth is MineCLI's first implemented registry source.

## Search

Search behaves like Modrinth discovery by default: it does not filter by the
server's loader, server-side metadata, or Minecraft version unless explicitly
requested.

```bash
minecli --server survival search farmers --kind mod --limit 50
minecli --server survival search fabric-api --kind mod --loader fabric
minecli --server survival search bluemap --server-compatible
minecli --server survival search farmers-delight-refabricated
minecli --server survival search farmers-delight-refabricated --minecraft 26.1.2
```

Use `--server-compatible` when you want MineCLI to filter using the current
server's loader/platform and server-side support. Add `--all-sides` with
`--server-compatible` to keep the server loader/platform filter but include
projects not marked useful on servers.

Search output prints the compatible game versions reported by Modrinth for each
result. Use `--minecraft <version>` when you want to restrict results to one
Minecraft version. Use `--loader <loader>` when you want to filter to a specific
Modrinth loader or platform such as `fabric`, `neoforge`, or `paper`.

## Install

Install resolves:

- project metadata
- compatible versions for the configured Minecraft version
- loader/platform compatibility
- required dependencies
- primary downloadable file

MineCLI can infer a better content kind when a project is broadly typed. For example, a project listed as `mod` can install as a Paper/Purpur plugin when a compatible Paper artifact exists.

## Import

`minecli import` matches existing files by SHA-512 through Modrinth's version-file hash lookup endpoint. Unmatched files remain unmanaged.

## Updates

`outdated` and `update` use the current server Minecraft version and loader. Local files, local folders, and modpack entries are intentionally skipped by registry update commands.

## Modpacks

`.mrpack` support reads `modrinth.index.json`, installs required server-side files, rejects client-only packs, and copies `overrides/` plus `server-overrides/` files.
