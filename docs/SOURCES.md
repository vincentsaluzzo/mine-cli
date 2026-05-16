# Package Sources

MineCLI keeps user-facing commands source-neutral, but each package in the lockfile records the source that owns its project and version identifiers.

## Current Priority

When the same package can eventually be resolved from more than one registry, MineCLI should prefer sources in this order:

1. Modrinth
2. Hangar
3. CurseForge
4. Local files and folders

This is intentionally conservative. Modrinth is already implemented and works without a per-user API key. Hangar is a strong next target for Paper, Velocity, and Waterfall plugins. CurseForge needs explicit API-key handling and terms-aware download behavior before it can be enabled.

## Lockfile Identity

`project_id` and `version_id` remain for backward compatibility. New entries also write:

- `source_project_id`
- `source_version_id`

The stable package identity is `source:source_project_id`. This prevents a future Hangar or CurseForge package from overwriting a Modrinth package that happens to use the same raw ID.

## Hangar Evaluation

Hangar is a good fit for plugin workflows:

- It is focused on Paper, Velocity, and Waterfall plugins.
- Public project pages expose platform and Minecraft-version compatibility.
- The site links to a Hangar API, and project/version pages expose enough concepts for MineCLI's model: project, version, platform, channel, and download artifact.

Open questions before implementation:

- Confirm stable REST endpoints and response schemas from the Hangar API.
- Confirm whether hash lookup is available. If not, `minecli import` cannot match existing Hangar files by hash and should only import Hangar packages through explicit install metadata.
- Map Hangar platforms to MineCLI server types: Paper covers Paper/Purpur/Folia where compatible; Velocity and Waterfall map directly.

References:

- https://hangar.papermc.io/
- https://hangar.papermc.io/HelpChat/PlaceholderAPI/versions

## CurseForge Evaluation

CurseForge is useful for the broader mod ecosystem, but it should not be enabled until API-key and download policy handling are explicit:

- The REST API uses `https://api.curseforge.com` and requires an `x-api-key` header.
- The API includes Minecraft-specific versions/loaders, mod search, files, changelogs, download URLs, and fingerprint matching.
- CurseForge API keys are issued to developers and are non-transferable. MineCLI must never embed or redistribute a shared key.

Implementation requirements:

- Add config/env support for a user-provided key, for example `MINECLI_CURSEFORGE_API_KEY`.
- Respect projects/files that do not expose third-party download URLs.
- Keep CurseForge disabled by default until credentials are configured.
- Prefer fingerprint matching for import once the hash/fingerprint algorithm is implemented.

References:

- https://docs.curseforge.com/rest-api/
- https://support.curseforge.com/support/solutions/articles/9000208346
- https://support.curseforge.com/support/solutions/articles/9000207405-curseforge-3rd-party-api-terms-and-conditions
