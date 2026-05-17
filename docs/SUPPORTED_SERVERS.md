# Supported Servers

MineCLI models three content kinds:

- `mod`
- `plugin`
- `datapack`

## Server Types

| Server type | Mods | Plugins | Datapacks |
| --- | --- | --- | --- |
| Vanilla | no | no | yes |
| Fabric | yes | no | yes |
| Quilt | yes | no | yes |
| Forge | yes | no | yes |
| NeoForge | yes | no | yes |
| Paper | no | yes | yes |
| Purpur | no | yes | yes |
| Spigot | no | yes | yes |
| Bukkit | no | yes | yes |
| Folia | no | yes | yes |
| Sponge | no | yes | yes |
| Velocity | no | yes | yes |
| Waterfall | no | yes | yes |
| BungeeCord | no | yes | yes |

## Detection

`minecli init` detects common layouts from:

- server jar names
- startup scripts
- `server.properties`
- world name from `level-name`
- known marker files such as `purpur.yml`, `paper.yml`, and `velocity.toml`

For Docker-managed Purpur/Geyser folders, MineCLI recognizes layouts used by `TheRemote/Legendary-Minecraft-Purpur-Geyser`, including `purpur.jar`, `purpur.yml`, `version_history.json`, `versions/`, and `cache/`.

## Paths

Default paths are:

- mods: `mods/`
- plugins: `plugins/`
- datapacks: `<world>/datapacks/`

These paths are stored in `.minecli/server.toml` and can be edited with:

```bash
minecli --server survival edit
```
