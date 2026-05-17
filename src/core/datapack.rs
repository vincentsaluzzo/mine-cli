use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use zip::ZipArchive;

use crate::core::lockfile::LockFile;
use crate::core::server::{ContentKind, ServerPaths};
use crate::error::{IoResultExt, MinecliError, Result};

pub const DISABLED_DATAPACKS_DIR: &str = "datapacks-disabled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatapackEntry {
    pub id: String,
    pub enabled: bool,
    pub path: PathBuf,
    pub pack_format: Option<i64>,
    pub description: Option<String>,
    pub managed: bool,
}

#[derive(Debug, Deserialize)]
struct PackMcmeta {
    pack: PackSection,
}

#[derive(Debug, Deserialize)]
struct PackSection {
    #[serde(rename = "pack_format")]
    pack_format: Option<i64>,
    description: Option<serde_json::Value>,
}

pub fn disabled_datapacks_path() -> PathBuf {
    PathBuf::from(crate::core::manifest::MINECLI_DIR).join(DISABLED_DATAPACKS_DIR)
}

pub fn discover_datapacks(
    server_dir: &Path,
    paths: &ServerPaths,
    lockfile: &LockFile,
) -> Result<Vec<DatapackEntry>> {
    let mut entries = Vec::new();
    collect_datapacks_from_dir(server_dir, &paths.datapacks, true, lockfile, &mut entries)?;
    collect_datapacks_from_dir(
        server_dir,
        &disabled_datapacks_path(),
        false,
        lockfile,
        &mut entries,
    )?;
    entries.sort_by(|left, right| {
        left.enabled
            .cmp(&right.enabled)
            .reverse()
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(entries)
}

pub fn disable_datapack(
    server_dir: &Path,
    paths: &ServerPaths,
    lockfile: &mut LockFile,
    query: &str,
) -> Result<DatapackEntry> {
    let entries = discover_datapacks(server_dir, paths, lockfile)?;
    let entry = find_datapack(&entries, query, true)?;
    let target_relative = disabled_datapacks_path().join(filename(&entry.path)?);
    move_datapack(server_dir, &entry.path, &target_relative)?;
    update_lockfile_path(lockfile, &entry.path, &target_relative);

    Ok(DatapackEntry {
        enabled: false,
        path: target_relative,
        ..entry
    })
}

pub fn enable_datapack(
    server_dir: &Path,
    paths: &ServerPaths,
    lockfile: &mut LockFile,
    query: &str,
) -> Result<DatapackEntry> {
    let entries = discover_datapacks(server_dir, paths, lockfile)?;
    let entry = find_datapack(&entries, query, false)?;
    let target_relative = paths.datapacks.join(filename(&entry.path)?);
    move_datapack(server_dir, &entry.path, &target_relative)?;
    update_lockfile_path(lockfile, &entry.path, &target_relative);

    Ok(DatapackEntry {
        enabled: true,
        path: target_relative,
        ..entry
    })
}

fn collect_datapacks_from_dir(
    server_dir: &Path,
    relative_dir: &Path,
    enabled: bool,
    lockfile: &LockFile,
    entries: &mut Vec<DatapackEntry>,
) -> Result<()> {
    let absolute_dir = server_dir.join(relative_dir);
    if !absolute_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&absolute_dir).at(&absolute_dir)? {
        let entry = entry.at(&absolute_dir)?;
        let absolute_path = entry.path();
        if !is_datapack_candidate(&absolute_path) {
            continue;
        }
        let relative_path = relative_dir.join(entry.file_name());
        let metadata = read_pack_mcmeta(&absolute_path).unwrap_or(None);
        let managed = lockfile.packages.iter().any(|package| {
            package.kind == ContentKind::Datapack && package.installed_path == relative_path
        });
        entries.push(DatapackEntry {
            id: datapack_id(&relative_path),
            enabled,
            path: relative_path,
            pack_format: metadata
                .as_ref()
                .and_then(|metadata| metadata.pack.pack_format),
            description: metadata
                .and_then(|metadata| description_to_string(metadata.pack.description)),
            managed,
        });
    }

    Ok(())
}

fn is_datapack_candidate(path: &Path) -> bool {
    path.is_dir()
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

fn read_pack_mcmeta(path: &Path) -> Result<Option<PackMcmeta>> {
    let mut contents = String::new();
    if path.is_dir() {
        let metadata_path = path.join("pack.mcmeta");
        if !metadata_path.exists() {
            return Ok(None);
        }
        contents = fs::read_to_string(&metadata_path).at(&metadata_path)?;
    } else {
        let file = File::open(path).at(path)?;
        let mut archive = ZipArchive::new(file).map_err(|error| {
            MinecliError::message(format!("failed to read datapack zip: {error}"))
        })?;
        let Ok(mut metadata) = archive.by_name("pack.mcmeta") else {
            return Ok(None);
        };
        metadata.read_to_string(&mut contents).map_err(|error| {
            MinecliError::message(format!("failed to read pack.mcmeta: {error}"))
        })?;
    }

    serde_json::from_str(&contents)
        .map(Some)
        .map_err(MinecliError::Json)
}

fn description_to_string(description: Option<serde_json::Value>) -> Option<String> {
    match description? {
        serde_json::Value::String(value) => Some(value),
        value => Some(value.to_string()),
    }
}

fn find_datapack(entries: &[DatapackEntry], query: &str, enabled: bool) -> Result<DatapackEntry> {
    entries
        .iter()
        .find(|entry| {
            entry.enabled == enabled
                && (entry.id == query
                    || entry.path == Path::new(query)
                    || filename(&entry.path)
                        .is_ok_and(|filename| filename.to_string_lossy() == query))
        })
        .cloned()
        .ok_or_else(|| {
            MinecliError::message(format!(
                "{} datapack `{query}` was not found",
                if enabled { "enabled" } else { "disabled" }
            ))
        })
}

fn move_datapack(server_dir: &Path, source: &Path, target: &Path) -> Result<()> {
    let absolute_source = server_dir.join(source);
    let absolute_target = server_dir.join(target);
    if absolute_target.exists() {
        return Err(MinecliError::message(format!(
            "target datapack already exists: {}",
            target.display()
        )));
    }
    if let Some(parent) = absolute_target.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    fs::rename(&absolute_source, &absolute_target).at(&absolute_target)
}

fn update_lockfile_path(lockfile: &mut LockFile, source: &Path, target: &Path) {
    for package in &mut lockfile.packages {
        if package.kind == ContentKind::Datapack && package.installed_path == source {
            package.installed_path = target.to_path_buf();
        }
    }
}

fn datapack_id(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn filename(path: &Path) -> Result<std::ffi::OsString> {
    path.file_name()
        .map(ToOwned::to_owned)
        .ok_or_else(|| MinecliError::message(format!("invalid datapack path: {}", path.display())))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::PathBuf;

    use zip::write::FileOptions;

    use crate::core::datapack::{disable_datapack, discover_datapacks, enable_datapack};
    use crate::core::lockfile::{LockFile, LockedPackage};
    use crate::core::server::{ContentKind, ServerPaths};

    #[test]
    fn discovers_folder_and_zip_datapacks() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ServerPaths::defaults("world");
        std::fs::create_dir_all(temp.path().join("world/datapacks/folder-pack")).unwrap();
        std::fs::write(
            temp.path().join("world/datapacks/folder-pack/pack.mcmeta"),
            r#"{"pack":{"pack_format":48,"description":"Folder pack"}}"#,
        )
        .unwrap();
        write_zip_pack(&temp.path().join("world/datapacks/zip-pack.zip"));

        let entries = discover_datapacks(temp.path(), &paths, &LockFile::default()).unwrap();

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.id == "folder-pack"));
        assert!(entries.iter().any(|entry| entry.id == "zip-pack"));
    }

    #[test]
    fn disable_and_enable_datapack_moves_file_and_updates_lockfile() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ServerPaths::defaults("world");
        std::fs::create_dir_all(temp.path().join("world/datapacks")).unwrap();
        write_zip_pack(&temp.path().join("world/datapacks/zip-pack.zip"));
        let mut lockfile = LockFile {
            packages: vec![datapack_package("world/datapacks/zip-pack.zip")],
        };

        let disabled = disable_datapack(temp.path(), &paths, &mut lockfile, "zip-pack").unwrap();

        assert!(!disabled.enabled);
        assert!(
            temp.path()
                .join(".minecli/datapacks-disabled/zip-pack.zip")
                .exists()
        );
        assert_eq!(
            lockfile.packages[0].installed_path,
            PathBuf::from(".minecli/datapacks-disabled/zip-pack.zip")
        );

        let enabled = enable_datapack(temp.path(), &paths, &mut lockfile, "zip-pack").unwrap();

        assert!(enabled.enabled);
        assert!(temp.path().join("world/datapacks/zip-pack.zip").exists());
        assert_eq!(
            lockfile.packages[0].installed_path,
            PathBuf::from("world/datapacks/zip-pack.zip")
        );
    }

    fn write_zip_pack(path: &std::path::Path) {
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file::<_, ()>("pack.mcmeta", FileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"pack":{"pack_format":48,"description":"Zip pack"}}"#)
            .unwrap();
        writer.finish().unwrap();
    }

    fn datapack_package(path: &str) -> LockedPackage {
        LockedPackage {
            source: "local-file".to_owned(),
            project_id: "local:zip-pack".to_owned(),
            source_project_id: Some("zip-pack".to_owned()),
            slug: "zip-pack".to_owned(),
            title: "zip-pack".to_owned(),
            kind: ContentKind::Datapack,
            loader: Some("datapack".to_owned()),
            version_id: "local".to_owned(),
            source_version_id: Some("local".to_owned()),
            version_number: "local".to_owned(),
            filename: "zip-pack.zip".to_owned(),
            hashes: BTreeMap::new(),
            installed_path: PathBuf::from(path),
            dependencies: vec![],
            installed_as_dependency: false,
        }
    }
}
