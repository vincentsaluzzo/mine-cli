# Modrinth Behavior

Modrinth is MineCLI's first implemented registry source.

## Search

Search uses the server's loader where possible, but does not filter by Minecraft
version unless explicitly requested:

```bash
minecli --server survival search bluemap
minecli --server survival search fabric-api --kind mod
minecli --server survival search minimap --all-sides
minecli --server survival search farmers-delight-refabricated
minecli --server survival search farmers-delight-refabricated --minecraft 26.1.2
```

By default, MineCLI hides projects not marked useful on servers. `--all-sides` removes that filter for inspection.

Search output prints the compatible game versions reported by Modrinth for each
result. Use `--minecraft <version>` when you want to restrict results to one
Minecraft version.

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
