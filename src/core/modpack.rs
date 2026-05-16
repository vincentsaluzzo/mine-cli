use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use zip::ZipArchive;

use crate::core::server::{ContentKind, ServerType};
use crate::error::{IoResultExt, MinecliError, Result};

pub const MODRINTH_INDEX_FILE: &str = "modrinth.index.json";

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthPackIndex {
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    pub game: String,
    #[serde(rename = "versionId")]
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<ModrinthPackFile>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

impl ModrinthPackIndex {
    pub fn required_server_files(&self) -> Vec<&ModrinthPackFile> {
        self.files
            .iter()
            .filter(|file| file.server_side() == SideSupport::Required)
            .collect()
    }

    pub fn optional_server_files(&self) -> Vec<&ModrinthPackFile> {
        self.files
            .iter()
            .filter(|file| file.server_side() == SideSupport::Optional)
            .collect()
    }

    pub fn is_client_only(&self) -> bool {
        !self.files.is_empty()
            && self
                .files
                .iter()
                .all(|file| file.server_side() == SideSupport::Unsupported)
    }

    pub fn validate_for_server(
        &self,
        server_type: ServerType,
        minecraft_version: &str,
    ) -> Result<()> {
        if self.format_version != 1 {
            return Err(MinecliError::message(format!(
                "unsupported Modrinth modpack format version {}",
                self.format_version
            )));
        }
        if self.game != "minecraft" {
            return Err(MinecliError::message(format!(
                "unsupported modpack game `{}`",
                self.game
            )));
        }
        if self.is_client_only() {
            return Err(MinecliError::message(format!(
                "{} is client-only and has no server-side files",
                self.name
            )));
        }
        if let Some(pack_minecraft) = self.dependencies.get("minecraft")
            && pack_minecraft != minecraft_version
        {
            return Err(MinecliError::message(format!(
                "{} targets Minecraft {}, but this server is {}",
                self.name, pack_minecraft, minecraft_version
            )));
        }
        if let Some(loader) = self.loader_dependency() {
            let compatible = match loader {
                "fabric-loader" => server_type == ServerType::Fabric,
                "quilt-loader" => server_type == ServerType::Quilt,
                "forge" => server_type == ServerType::Forge,
                "neoforge" => server_type == ServerType::NeoForge,
                _ => true,
            };
            if !compatible {
                return Err(MinecliError::message(format!(
                    "{} requires {}, but this server is {}",
                    self.name, loader, server_type
                )));
            }
        }

        Ok(())
    }

    pub fn loader_dependency(&self) -> Option<&str> {
        ["fabric-loader", "quilt-loader", "forge", "neoforge"]
            .into_iter()
            .find(|loader| self.dependencies.contains_key(*loader))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthPackFile {
    pub path: PathBuf,
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub env: Option<ModrinthPackEnv>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default, rename = "fileSize")]
    pub file_size: u64,
}

impl ModrinthPackFile {
    pub fn server_side(&self) -> SideSupport {
        self.env
            .as_ref()
            .and_then(|env| env.server)
            .unwrap_or(SideSupport::Required)
    }

    pub fn content_kind(&self) -> Option<ContentKind> {
        let first = self.path.components().next()?.as_os_str().to_string_lossy();
        match first.as_ref() {
            "mods" => Some(ContentKind::Mod),
            "plugins" => Some(ContentKind::Plugin),
            _ if self
                .path
                .components()
                .any(|component| component.as_os_str() == "datapacks") =>
            {
                Some(ContentKind::Datapack)
            }
            _ => None,
        }
    }

    pub fn filename(&self) -> Result<String> {
        self.path
            .file_name()
            .and_then(|filename| filename.to_str())
            .map(ToOwned::to_owned)
            .ok_or_else(|| MinecliError::message("modpack file path has no valid filename"))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ModrinthPackEnv {
    #[allow(dead_code)]
    pub client: Option<SideSupport>,
    pub server: Option<SideSupport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideSupport {
    Required,
    Optional,
    Unsupported,
}

pub fn read_modrinth_pack(path: &Path) -> Result<ModrinthPackIndex> {
    let file = File::open(path).at(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| MinecliError::message(format!("failed to read mrpack zip: {error}")))?;
    let mut index = archive.by_name(MODRINTH_INDEX_FILE).map_err(|error| {
        MinecliError::message(format!("missing {MODRINTH_INDEX_FILE}: {error}"))
    })?;
    let mut contents = String::new();
    index.read_to_string(&mut contents).map_err(|error| {
        MinecliError::message(format!("failed to read {MODRINTH_INDEX_FILE}: {error}"))
    })?;
    serde_json::from_str(&contents).map_err(MinecliError::Json)
}

pub fn copy_server_overrides(pack_path: &Path, server_dir: &Path) -> Result<usize> {
    let file = File::open(pack_path).at(pack_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| MinecliError::message(format!("failed to read mrpack zip: {error}")))?;
    let mut copied = 0usize;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            MinecliError::message(format!("failed to read mrpack entry: {error}"))
        })?;
        let name = file.name().to_owned();
        let Some(relative) = name
            .strip_prefix("server-overrides/")
            .or_else(|| name.strip_prefix("overrides/"))
        else {
            continue;
        };
        if relative.is_empty() || file.is_dir() {
            continue;
        }
        let relative = PathBuf::from(relative);
        validate_relative_path(&relative)?;
        let target = server_dir.join(&relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).at(parent)?;
        }
        let mut output = File::create(&target).at(&target)?;
        std::io::copy(&mut file, &mut output).at(&target)?;
        copied += 1;
    }

    Ok(copied)
}

pub fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(MinecliError::message(format!(
            "unsafe modpack path: {}",
            path.display()
        )));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    }) {
        return Err(MinecliError::message(format!(
            "unsafe modpack path: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::FileOptions;

    use crate::core::modpack::{copy_server_overrides, read_modrinth_pack, validate_relative_path};
    use crate::core::server::ServerType;

    #[test]
    fn parses_server_side_files_from_mrpack() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack.mrpack");
        write_pack(&pack, sample_index());

        let index = read_modrinth_pack(&pack).unwrap();

        assert_eq!(index.name, "Test Pack");
        assert_eq!(index.required_server_files().len(), 1);
        assert_eq!(index.optional_server_files().len(), 1);
        assert!(!index.is_client_only());
        index
            .validate_for_server(ServerType::Fabric, "1.21.5")
            .unwrap();
    }

    #[test]
    fn detects_client_only_mrpack() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack.mrpack");
        write_pack(
            &pack,
            r#"{
                "formatVersion": 1,
                "game": "minecraft",
                "versionId": "client",
                "name": "Client Pack",
                "files": [{
                    "path": "mods/client.jar",
                    "hashes": {"sha512": "abc"},
                    "downloads": ["https://cdn.modrinth.com/data/a/versions/b/client.jar"],
                    "fileSize": 1,
                    "env": {"client": "required", "server": "unsupported"}
                }],
                "dependencies": {"minecraft": "1.21.5", "fabric-loader": "0.16.0"}
            }"#,
        );

        let index = read_modrinth_pack(&pack).unwrap();

        assert!(index.is_client_only());
        assert!(
            index
                .validate_for_server(ServerType::Fabric, "1.21.5")
                .unwrap_err()
                .to_string()
                .contains("client-only")
        );
    }

    #[test]
    fn copies_safe_server_overrides() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack.mrpack");
        let file = std::fs::File::create(&pack).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file::<_, ()>("modrinth.index.json", FileOptions::default())
            .unwrap();
        writer.write_all(sample_index().as_bytes()).unwrap();
        writer
            .start_file::<_, ()>(
                "server-overrides/config/server.toml",
                FileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"config").unwrap();
        writer.finish().unwrap();

        let copied = copy_server_overrides(&pack, temp.path()).unwrap();

        assert_eq!(copied, 1);
        assert_eq!(
            std::fs::read(temp.path().join("config/server.toml")).unwrap(),
            b"config"
        );
    }

    #[test]
    fn rejects_unsafe_modpack_paths() {
        assert!(validate_relative_path(std::path::Path::new("../server.properties")).is_err());
    }

    fn write_pack(path: &std::path::Path, index: &str) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file::<_, ()>("modrinth.index.json", FileOptions::default())
            .unwrap();
        writer.write_all(index.as_bytes()).unwrap();
        writer.finish().unwrap();
    }

    fn sample_index() -> &'static str {
        r#"{
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "pack-version",
            "name": "Test Pack",
            "summary": "A server-compatible pack",
            "files": [
                {
                    "path": "mods/server.jar",
                    "hashes": {"sha512": "abc"},
                    "downloads": ["https://cdn.modrinth.com/data/a/versions/b/server.jar"],
                    "fileSize": 12,
                    "env": {"client": "required", "server": "required"}
                },
                {
                    "path": "mods/optional.jar",
                    "hashes": {"sha512": "def"},
                    "downloads": ["https://cdn.modrinth.com/data/a/versions/b/optional.jar"],
                    "fileSize": 13,
                    "env": {"client": "optional", "server": "optional"}
                },
                {
                    "path": "mods/client.jar",
                    "hashes": {"sha512": "ghi"},
                    "downloads": ["https://cdn.modrinth.com/data/a/versions/b/client.jar"],
                    "fileSize": 14,
                    "env": {"client": "required", "server": "unsupported"}
                }
            ],
            "dependencies": {"minecraft": "1.21.5", "fabric-loader": "0.16.0"}
        }"#
    }
}
