use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::error::{IoResultExt, MinecliError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ContentKind {
    Mod,
    Plugin,
    Datapack,
}

impl fmt::Display for ContentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mod => formatter.write_str("mod"),
            Self::Plugin => formatter.write_str("plugin"),
            Self::Datapack => formatter.write_str("datapack"),
        }
    }
}

impl FromStr for ContentKind {
    type Err = MinecliError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "mod" => Ok(Self::Mod),
            "plugin" => Ok(Self::Plugin),
            "datapack" => Ok(Self::Datapack),
            other => Err(MinecliError::message(format!(
                "unsupported content kind `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ServerType {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    #[value(name = "neoforge")]
    NeoForge,
    Paper,
    Purpur,
    Spigot,
    Bukkit,
    Folia,
    Sponge,
    Velocity,
    Waterfall,
    #[serde(rename = "bungeecord")]
    #[value(name = "bungeecord", alias = "bungee-cord")]
    BungeeCord,
    Unknown,
}

impl fmt::Display for ServerType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ServerType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Paper => "paper",
            Self::Purpur => "purpur",
            Self::Spigot => "spigot",
            Self::Bukkit => "bukkit",
            Self::Folia => "folia",
            Self::Sponge => "sponge",
            Self::Velocity => "velocity",
            Self::Waterfall => "waterfall",
            Self::BungeeCord => "bungeecord",
            Self::Unknown => "unknown",
        }
    }

    pub fn supports(self, kind: ContentKind) -> bool {
        match kind {
            ContentKind::Datapack => true,
            ContentKind::Mod => matches!(
                self,
                Self::Fabric | Self::Quilt | Self::Forge | Self::NeoForge
            ),
            ContentKind::Plugin => matches!(
                self,
                Self::Paper
                    | Self::Purpur
                    | Self::Spigot
                    | Self::Bukkit
                    | Self::Folia
                    | Self::Sponge
                    | Self::Velocity
                    | Self::Waterfall
                    | Self::BungeeCord
            ),
        }
    }

    pub fn modrinth_loader(self, kind: ContentKind) -> Option<&'static str> {
        match kind {
            ContentKind::Datapack => Some("datapack"),
            ContentKind::Mod if self.supports(kind) => Some(self.as_str()),
            ContentKind::Plugin if self.supports(kind) => Some(match self {
                Self::Purpur | Self::Folia => "paper",
                Self::Bukkit => "spigot",
                server_type => server_type.as_str(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerPaths {
    pub mods: PathBuf,
    pub plugins: PathBuf,
    pub datapacks: PathBuf,
}

impl ServerPaths {
    pub fn defaults(world: &str) -> Self {
        Self {
            mods: PathBuf::from("mods"),
            plugins: PathBuf::from("plugins"),
            datapacks: PathBuf::from(world).join("datapacks"),
        }
    }

    pub fn target_for(&self, kind: ContentKind) -> &Path {
        match kind {
            ContentKind::Mod => &self.mods,
            ContentKind::Plugin => &self.plugins,
            ContentKind::Datapack => &self.datapacks,
        }
    }
}

pub fn detect_server_type(server_dir: &Path) -> Result<ServerType> {
    let entries = match fs::read_dir(server_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ServerType::Unknown);
        }
        Err(error) => {
            return Err(MinecliError::Io {
                path: server_dir.to_path_buf(),
                source: error,
            });
        }
    };

    let mut detected = ServerType::Unknown;
    for entry in entries {
        let entry = entry.at(server_dir)?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".jar") {
            continue;
        }

        detected = if name.contains("fabric-server-launch") || name.contains("fabric-server") {
            ServerType::Fabric
        } else if name.starts_with("neoforge") || name.contains("neoforge-") {
            ServerType::NeoForge
        } else if name.starts_with("forge") || name.contains("forge-") {
            ServerType::Forge
        } else if name.starts_with("paper") || name.contains("paper-") {
            ServerType::Paper
        } else if name.starts_with("purpur") || name.contains("purpur-") {
            ServerType::Purpur
        } else if name.starts_with("spigot") || name.contains("spigot-") {
            ServerType::Spigot
        } else {
            detected
        };

        if detected != ServerType::Unknown {
            break;
        }
    }

    Ok(detected)
}

pub fn detect_world_name(server_dir: &Path) -> Result<String> {
    let properties_path = server_dir.join("server.properties");
    let properties = match fs::read_to_string(&properties_path) {
        Ok(properties) => properties,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok("world".to_owned()),
        Err(error) => {
            return Err(MinecliError::Io {
                path: properties_path,
                source: error,
            });
        }
    };

    for line in properties.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("level-name=") {
            let value = value.trim();
            if !value.is_empty() {
                return Ok(value.to_owned());
            }
        }
    }

    Ok("world".to_owned())
}

pub fn content_kind_from_project_type(project_type: &str) -> Result<ContentKind> {
    match project_type {
        "mod" => Ok(ContentKind::Mod),
        "plugin" => Ok(ContentKind::Plugin),
        "datapack" => Ok(ContentKind::Datapack),
        other => Err(MinecliError::message(format!(
            "unsupported Modrinth project type `{other}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentKind, ServerPaths, ServerType, detect_server_type, detect_world_name};

    #[test]
    fn resolves_default_content_paths() {
        let paths = ServerPaths::defaults("overworld");

        assert_eq!(
            paths.target_for(ContentKind::Mod),
            std::path::Path::new("mods")
        );
        assert_eq!(
            paths.target_for(ContentKind::Plugin),
            std::path::Path::new("plugins")
        );
        assert_eq!(
            paths.target_for(ContentKind::Datapack),
            std::path::Path::new("overworld/datapacks")
        );
    }

    #[test]
    fn validates_server_type_content_support() {
        assert!(ServerType::Fabric.supports(ContentKind::Mod));
        assert!(!ServerType::Fabric.supports(ContentKind::Plugin));
        assert!(ServerType::Paper.supports(ContentKind::Plugin));
        assert!(ServerType::Vanilla.supports(ContentKind::Datapack));
    }

    #[test]
    fn detects_server_type_from_jar_name() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("paper-1.21.5.jar"), "").unwrap();

        assert_eq!(detect_server_type(temp.path()).unwrap(), ServerType::Paper);
    }

    #[test]
    fn detects_world_name_from_server_properties() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("server.properties"),
            "level-name=survival\n",
        )
        .unwrap();

        assert_eq!(detect_world_name(temp.path()).unwrap(), "survival");
    }
}
