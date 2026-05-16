use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::core::lockfile::{LockedPackage, load_lockfile, write_lockfile};
use crate::core::manifest::{minecli_dir, write_atomic};
use crate::error::{IoResultExt, MinecliError, Result};

pub const BACKUPS_DIR: &str = "backups";
pub const METADATA_FILE: &str = "metadata.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupOperation {
    pub id: String,
    pub created_at_unix: u64,
    pub action: String,
    #[serde(default)]
    pub files: Vec<BackupFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFile {
    pub package: LockedPackage,
    pub backup_path: PathBuf,
}

pub fn backups_dir(server_dir: &Path) -> PathBuf {
    minecli_dir(server_dir).join(BACKUPS_DIR)
}

pub fn create_backup_operation(
    server_dir: &Path,
    action: impl Into<String>,
    packages: &[LockedPackage],
) -> Result<Option<BackupOperation>> {
    let mut files = Vec::new();
    for package in packages {
        let source_path = server_dir.join(&package.installed_path);
        if !source_path.exists() {
            continue;
        }
        files.push(package.clone());
    }

    if files.is_empty() {
        return Ok(None);
    }

    let id = operation_id()?;
    let operation_dir = backups_dir(server_dir).join(&id);
    let mut operation = BackupOperation {
        id: id.clone(),
        created_at_unix: now_unix_seconds()?,
        action: action.into(),
        files: Vec::new(),
    };

    for package in files {
        let source_path = server_dir.join(&package.installed_path);
        let backup_path = PathBuf::from("files").join(&package.installed_path);
        let absolute_backup_path = operation_dir.join(&backup_path);
        if let Some(parent) = absolute_backup_path.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::copy(&source_path, &absolute_backup_path).at(&absolute_backup_path)?;
        operation.files.push(BackupFile {
            package,
            backup_path,
        });
    }

    write_backup_metadata(server_dir, &operation)?;
    Ok(Some(operation))
}

pub fn list_backup_operations(server_dir: &Path) -> Result<Vec<BackupOperation>> {
    let dir = backups_dir(server_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut operations = Vec::new();
    for entry in fs::read_dir(&dir).at(&dir)? {
        let entry = entry.at(&dir)?;
        let path = entry.path().join(METADATA_FILE);
        if !path.exists() {
            continue;
        }
        operations.push(read_backup_metadata_path(&path)?);
    }
    operations.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(operations)
}

pub fn read_backup_operation(server_dir: &Path, id: &str) -> Result<BackupOperation> {
    let path = backups_dir(server_dir).join(id).join(METADATA_FILE);
    read_backup_metadata_path(&path)
}

pub fn rollback_operation(server_dir: &Path, id: &str) -> Result<BackupOperation> {
    let operation = read_backup_operation(server_dir, id)?;
    let mut lockfile = load_lockfile(server_dir)?;
    let operation_dir = backups_dir(server_dir).join(id);

    for file in &operation.files {
        let backup_path = operation_dir.join(&file.backup_path);
        if !backup_path.is_file() {
            return Err(MinecliError::message(format!(
                "backup file is missing: {}",
                backup_path.display()
            )));
        }
        if !is_safe_relative_path(&file.package.installed_path) {
            return Err(MinecliError::message(format!(
                "backup target is invalid for {}",
                file.package.slug
            )));
        }
    }

    for file in &operation.files {
        if let Some(current) = lockfile.package_by_project_id(&file.package.project_id) {
            if current.installed_path != file.package.installed_path {
                let current_path = server_dir.join(&current.installed_path);
                match fs::remove_file(&current_path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(MinecliError::Io {
                            path: current_path,
                            source: error,
                        });
                    }
                }
            }
        }

        let backup_path = operation_dir.join(&file.backup_path);
        let target_path = server_dir.join(&file.package.installed_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::copy(&backup_path, &target_path).at(&target_path)?;
        lockfile.upsert_package(file.package.clone());
    }

    write_lockfile(server_dir, &lockfile)?;
    Ok(operation)
}

fn write_backup_metadata(server_dir: &Path, operation: &BackupOperation) -> Result<()> {
    let path = backups_dir(server_dir)
        .join(&operation.id)
        .join(METADATA_FILE);
    let contents =
        toml::to_string_pretty(operation).map_err(|source| MinecliError::TomlSerialize {
            path: path.clone(),
            source,
        })?;
    write_atomic(&path, contents.as_bytes())
}

fn read_backup_metadata_path(path: &Path) -> Result<BackupOperation> {
    let contents = fs::read_to_string(path).at(path)?;
    toml::from_str(&contents).map_err(|source| MinecliError::TomlDeserialize {
        path: path.to_path_buf(),
        source,
    })
}

fn operation_id() -> Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| MinecliError::message(format!("system clock error: {error}")))?
        .as_millis();
    Ok(format!("{millis}-{}", std::process::id()))
}

fn now_unix_seconds() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| MinecliError::message(format!("system clock error: {error}")))?
        .as_secs())
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::core::backups::{
        create_backup_operation, list_backup_operations, rollback_operation,
    };
    use crate::core::lockfile::{LockFile, LockedPackage, load_lockfile, write_lockfile};
    use crate::core::server::ContentKind;

    #[test]
    fn creates_backup_and_rolls_back_package_file_and_lockfile_entry() {
        let temp = tempfile::tempdir().unwrap();
        let server_dir = temp.path();
        let old_package = package("root", "1.0.0", "mods/root-old.jar");
        std::fs::create_dir_all(server_dir.join("mods")).unwrap();
        std::fs::write(server_dir.join("mods/root-old.jar"), b"old").unwrap();
        write_lockfile(
            server_dir,
            &LockFile {
                packages: vec![old_package.clone()],
            },
        )
        .unwrap();

        let backup = create_backup_operation(server_dir, "update root", &[old_package])
            .unwrap()
            .unwrap();
        std::fs::remove_file(server_dir.join("mods/root-old.jar")).unwrap();
        std::fs::write(server_dir.join("mods/root-new.jar"), b"new").unwrap();
        write_lockfile(
            server_dir,
            &LockFile {
                packages: vec![package("root", "2.0.0", "mods/root-new.jar")],
            },
        )
        .unwrap();

        rollback_operation(server_dir, &backup.id).unwrap();

        assert_eq!(
            std::fs::read(server_dir.join("mods/root-old.jar")).unwrap(),
            b"old"
        );
        assert!(!server_dir.join("mods/root-new.jar").exists());
        let lockfile = load_lockfile(server_dir).unwrap();
        let package = lockfile.package_by_project_id("root").unwrap();
        assert_eq!(package.version_number, "1.0.0");
        assert_eq!(list_backup_operations(server_dir).unwrap().len(), 1);
    }

    fn package(project_id: &str, version: &str, path: &str) -> LockedPackage {
        LockedPackage {
            source: "modrinth".to_owned(),
            project_id: project_id.to_owned(),
            slug: project_id.to_owned(),
            title: project_id.to_owned(),
            kind: ContentKind::Mod,
            loader: Some("fabric".to_owned()),
            version_id: version.to_owned(),
            version_number: version.to_owned(),
            filename: PathBuf::from(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            hashes: BTreeMap::new(),
            installed_path: PathBuf::from(path),
            dependencies: vec![],
            installed_as_dependency: false,
        }
    }
}
