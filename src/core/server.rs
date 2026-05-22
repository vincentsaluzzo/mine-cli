use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        self.modrinth_loaders(kind).first().copied()
    }

    pub fn modrinth_loaders(self, kind: ContentKind) -> Vec<&'static str> {
        match kind {
            ContentKind::Datapack => vec!["datapack"],
            ContentKind::Mod if self.supports(kind) => vec![self.as_str()],
            ContentKind::Plugin if self.supports(kind) => match self {
                Self::Purpur => vec!["paper", "spigot", "bukkit"],
                Self::Paper | Self::Folia => vec!["paper", "spigot", "bukkit"],
                Self::Spigot => vec!["spigot", "bukkit"],
                Self::Bukkit => vec!["bukkit"],
                server_type => vec![server_type.as_str()],
            },
            _ => Vec::new(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerDetection {
    pub server_type: ServerType,
    pub minecraft_version: Option<String>,
    pub world: String,
    pub paths: ServerPaths,
}

pub fn detect_server(server_dir: &Path) -> Result<ServerDetection> {
    let world = detect_world_name(server_dir)?;
    let server_type = detect_server_type_from_layout(server_dir)?;
    let minecraft_version = detect_minecraft_version(server_dir)?;
    let paths = ServerPaths::defaults(&world);

    Ok(ServerDetection {
        server_type,
        minecraft_version,
        world,
        paths,
    })
}

fn detect_server_type_from_layout(server_dir: &Path) -> Result<ServerType> {
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

    let mut script_candidates = Vec::new();
    for entry in entries {
        let entry = entry.at(server_dir)?;
        let name = entry.file_name().to_string_lossy().to_lowercase();

        if name.ends_with(".jar") {
            let detected = detect_server_type_from_name(&name);
            if detected != ServerType::Unknown {
                return Ok(detected);
            }
        } else if is_startup_script(&name) {
            script_candidates.push(entry.path());
        }
    }

    for script in script_candidates {
        if let Some(detected) = detect_server_type_from_script(&script)? {
            return Ok(detected);
        }
    }

    for nested in ["server", "servers", "run"] {
        let nested_path = server_dir.join(nested);
        if nested_path.is_dir() {
            let detected = detect_server_type_from_layout(&nested_path)?;
            if detected != ServerType::Unknown {
                return Ok(detected);
            }
        }
    }

    for marker in [
        ("purpur.yml", ServerType::Purpur),
        ("paper.yml", ServerType::Paper),
        ("spigot.yml", ServerType::Spigot),
        ("bukkit.yml", ServerType::Bukkit),
        ("velocity.toml", ServerType::Velocity),
        ("waterfall.yml", ServerType::Waterfall),
    ] {
        if server_dir.join(marker.0).exists() {
            return Ok(marker.1);
        }
    }

    if server_dir.join("server.properties").exists() {
        Ok(ServerType::Vanilla)
    } else {
        Ok(ServerType::Unknown)
    }
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

pub fn detect_minecraft_version(server_dir: &Path) -> Result<Option<String>> {
    if let Some(version) = detect_minecraft_version_from_history(server_dir)? {
        return Ok(Some(version));
    }

    let entries = match fs::read_dir(server_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(MinecliError::Io {
                path: server_dir.to_path_buf(),
                source: error,
            });
        }
    };

    for entry in entries {
        let entry = entry.at(server_dir)?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".jar") {
            continue;
        }

        if let Some(version) = minecraft_version_from_filename(&name) {
            return Ok(Some(version));
        }

        if let Some(version) = minecraft_version_from_jar(&path)? {
            return Ok(Some(version));
        }
    }

    detect_minecraft_version_from_nested_dirs(server_dir)
}

pub fn content_kind_from_project_type(project_type: &str) -> Result<ContentKind> {
    match project_type {
        "mod" => Ok(ContentKind::Mod),
        "plugin" => Ok(ContentKind::Plugin),
        "datapack" => Ok(ContentKind::Datapack),
        other => Err(MinecliError::message(format!(
            "unsupported package project type `{other}`"
        ))),
    }
}

fn detect_server_type_from_name(name: &str) -> ServerType {
    if name == "purpur.jar" {
        ServerType::Purpur
    } else if name == "paper.jar" {
        ServerType::Paper
    } else if name == "spigot.jar" {
        ServerType::Spigot
    } else if name == "bukkit.jar" {
        ServerType::Bukkit
    } else if name == "velocity.jar" {
        ServerType::Velocity
    } else if name == "waterfall.jar" {
        ServerType::Waterfall
    } else if name.contains("fabric-server-launch") || name.contains("fabric-server") {
        ServerType::Fabric
    } else if name.starts_with("quilt") || name.contains("quilt-") || name.contains("quilt_server")
    {
        ServerType::Quilt
    } else if name.starts_with("neoforge") || name.contains("neoforge-") {
        ServerType::NeoForge
    } else if name.starts_with("forge") || name.contains("forge-") {
        ServerType::Forge
    } else if name.starts_with("paper") || name.contains("paper-") {
        ServerType::Paper
    } else if name.starts_with("purpur") || name.contains("purpur-") {
        ServerType::Purpur
    } else if name.starts_with("folia") || name.contains("folia-") {
        ServerType::Folia
    } else if name.starts_with("spigot") || name.contains("spigot-") {
        ServerType::Spigot
    } else if name.starts_with("bukkit") || name.contains("bukkit-") {
        ServerType::Bukkit
    } else if name.starts_with("sponge") || name.contains("sponge") {
        ServerType::Sponge
    } else if name.starts_with("velocity") || name.contains("velocity-") {
        ServerType::Velocity
    } else if name.starts_with("waterfall") || name.contains("waterfall-") {
        ServerType::Waterfall
    } else if name.starts_with("bungeecord")
        || name.contains("bungeecord-")
        || name.contains("bungee-cord")
    {
        ServerType::BungeeCord
    } else {
        ServerType::Unknown
    }
}

fn detect_server_type_from_script(path: &Path) -> Result<Option<ServerType>> {
    let contents = fs::read_to_string(path).at(path)?;
    for token in contents
        .split(|character: char| character.is_whitespace() || matches!(character, '"' | '\''))
        .map(|token| token.trim().to_lowercase())
    {
        if !token.ends_with(".jar") {
            continue;
        }
        let filename = Path::new(&token)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&token);
        let detected = detect_server_type_from_name(filename);
        if detected != ServerType::Unknown {
            return Ok(Some(detected));
        }
    }

    Ok(None)
}

fn is_startup_script(name: &str) -> bool {
    matches!(
        name,
        "start.sh"
            | "run.sh"
            | "launch.sh"
            | "start.command"
            | "run.command"
            | "start.bat"
            | "run.bat"
            | "launch.bat"
    )
}

fn minecraft_version_from_filename(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    for index in 0..bytes.len() {
        if !bytes[index].is_ascii_digit() {
            continue;
        }
        let candidate = &name[index..];
        let version = candidate
            .chars()
            .take_while(|character| character.is_ascii_digit() || *character == '.')
            .collect::<String>();
        if version.matches('.').count() >= 1 && version.len() >= 4 {
            return Some(version.trim_end_matches('.').to_owned());
        }
    }
    None
}

fn detect_minecraft_version_from_history(server_dir: &Path) -> Result<Option<String>> {
    let path = server_dir.join("version_history.json");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(MinecliError::Io {
                path,
                source: error,
            });
        }
    };

    let json = match serde_json::from_str::<Value>(&contents) {
        Ok(json) => json,
        Err(_) => return Ok(None),
    };

    for key in ["currentVersion", "oldVersion"] {
        if let Some(value) = json.get(key).and_then(|value| value.as_str()) {
            if let Some(version) = minecraft_version_from_text(value) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

fn detect_minecraft_version_from_nested_dirs(server_dir: &Path) -> Result<Option<String>> {
    let mut candidates = Vec::new();
    for nested in ["versions", "cache"] {
        let nested_dir = server_dir.join(nested);
        if !nested_dir.is_dir() {
            continue;
        }
        collect_versions_from_dir(&nested_dir, &mut candidates)?;
    }

    candidates.sort_by(|left, right| compare_versions(left, right));
    Ok(candidates.pop())
}

fn collect_versions_from_dir(directory: &Path, candidates: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(directory).at(directory)?;
    for entry in entries {
        let entry = entry.at(directory)?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_dir() {
            if let Some(version) = minecraft_version_from_text(&name) {
                candidates.push(version);
            }
            collect_versions_from_dir(&path, candidates)?;
        } else if name.ends_with(".jar")
            && let Some(version) = minecraft_version_from_filename(&name)
        {
            candidates.push(version);
        }
    }

    Ok(())
}

fn minecraft_version_from_text(text: &str) -> Option<String> {
    if let Some(start) = text.find("(MC: ") {
        let after_marker = &text[start + 5..];
        if let Some(end) = after_marker.find(')') {
            return Some(after_marker[..end].trim().to_owned());
        }
    }

    minecraft_version_from_filename(text)
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);
    left_parts.cmp(&right_parts)
}

fn version_parts(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

fn minecraft_version_from_jar(path: &Path) -> Result<Option<String>> {
    let mut file = fs::File::open(path).at(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).at(path)?;

    if let Some(contents) = zip_entry(&bytes, "version.json") {
        if let Ok(json) = serde_json::from_slice::<Value>(&contents) {
            if let Some(id) = json.get("id").and_then(|id| id.as_str()) {
                return Ok(Some(id.to_owned()));
            }
        }
    }

    Ok(None)
}

fn zip_entry(bytes: &[u8], entry_name: &str) -> Option<Vec<u8>> {
    let mut index = 0;
    while index + 30 <= bytes.len() {
        if &bytes[index..index + 4] != b"PK\x03\x04" {
            index += 1;
            continue;
        }

        let compression = read_u16(bytes, index + 8)?;
        let compressed_size = read_u32(bytes, index + 18)? as usize;
        let uncompressed_size = read_u32(bytes, index + 22)? as usize;
        let filename_length = read_u16(bytes, index + 26)? as usize;
        let extra_length = read_u16(bytes, index + 28)? as usize;
        let filename_start = index + 30;
        let filename_end = filename_start + filename_length;
        let data_start = filename_end + extra_length;
        let data_end = data_start + compressed_size;
        if data_end > bytes.len() {
            return None;
        }

        let filename = std::str::from_utf8(&bytes[filename_start..filename_end]).ok()?;
        if filename == entry_name {
            return match compression {
                0 if compressed_size == uncompressed_size => {
                    Some(bytes[data_start..data_end].to_vec())
                }
                _ => None,
            };
        }

        index = data_end;
    }

    None
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ]))
}

#[cfg(test)]
mod tests {
    use super::{
        ContentKind, ServerPaths, ServerType, detect_minecraft_version, detect_server,
        detect_world_name,
    };

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

        assert_eq!(
            detect_server(temp.path()).unwrap().server_type,
            ServerType::Paper
        );
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

    #[test]
    fn detects_server_type_from_startup_script() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("start.sh"),
            "java -jar ./jars/purpur-1.21.5.jar",
        )
        .unwrap();

        assert_eq!(
            detect_server(temp.path()).unwrap().server_type,
            ServerType::Purpur
        );
    }

    #[test]
    fn detects_server_type_from_nested_run_folder() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("run")).unwrap();
        std::fs::write(temp.path().join("run/neoforge-21.5.97-installer.jar"), "").unwrap();

        assert_eq!(
            detect_server(temp.path()).unwrap().server_type,
            ServerType::NeoForge
        );
    }

    #[test]
    fn detects_vanilla_when_only_server_properties_exists() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("server.properties"), "").unwrap();

        assert_eq!(
            detect_server(temp.path()).unwrap().server_type,
            ServerType::Vanilla
        );
    }

    #[test]
    fn detects_minecraft_version_from_jar_filename() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("paper-1.21.5-123.jar"), "").unwrap();

        assert_eq!(
            detect_minecraft_version(temp.path()).unwrap().as_deref(),
            Some("1.21.5")
        );
    }

    #[test]
    fn detects_server_type_from_generic_purpur_jar() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("purpur.jar"), "").unwrap();

        assert_eq!(
            detect_server(temp.path()).unwrap().server_type,
            ServerType::Purpur
        );
    }

    #[test]
    fn detects_minecraft_version_from_version_history() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("version_history.json"),
            r#"{"currentVersion":"26.1.2-2585-8026cdb (MC: 26.1.2)"}"#,
        )
        .unwrap();

        assert_eq!(
            detect_minecraft_version(temp.path()).unwrap().as_deref(),
            Some("26.1.2")
        );
    }

    #[test]
    fn detects_latest_minecraft_version_from_versions_directory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("versions/1.21.3")).unwrap();
        std::fs::create_dir_all(temp.path().join("versions/1.21.11")).unwrap();
        std::fs::write(temp.path().join("versions/1.21.3/purpur-1.21.3.jar"), "").unwrap();
        std::fs::write(temp.path().join("versions/1.21.11/purpur-1.21.11.jar"), "").unwrap();

        assert_eq!(
            detect_minecraft_version(temp.path()).unwrap().as_deref(),
            Some("1.21.11")
        );
    }

    #[test]
    fn detect_server_returns_paths_from_properties_world() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("paper-1.21.5.jar"), "").unwrap();
        std::fs::write(
            temp.path().join("server.properties"),
            "level-name=worlds/survival\n",
        )
        .unwrap();

        let detection = detect_server(temp.path()).unwrap();

        assert_eq!(detection.server_type, ServerType::Paper);
        assert_eq!(detection.world, "worlds/survival");
        assert_eq!(
            detection.paths.datapacks,
            std::path::Path::new("worlds/survival/datapacks")
        );
    }
}
