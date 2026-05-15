use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::manifest::{minecli_dir, write_atomic};
use crate::core::server::ContentKind;
use crate::error::{IoResultExt, MinecliError, Result};

pub const LOCK_FILE: &str = "lock.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockFile {
    #[serde(default)]
    pub packages: Vec<LockedPackage>,
}

impl LockFile {
    pub fn package_by_project_id(&self, project_id: &str) -> Option<&LockedPackage> {
        self.packages
            .iter()
            .find(|package| package.project_id == project_id)
    }

    pub fn package_by_query(&self, query: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|package| {
            package.slug == query
                || package.project_id == query
                || package.filename == query
                || package.installed_path == Path::new(query)
        })
    }

    pub fn upsert_package(&mut self, package: LockedPackage) {
        if let Some(existing) = self
            .packages
            .iter_mut()
            .find(|existing| existing.project_id == package.project_id)
        {
            *existing = package;
        } else {
            self.packages.push(package);
        }
        self.packages
            .sort_by(|left, right| left.slug.cmp(&right.slug));
    }

    pub fn remove_project(&mut self, project_id: &str) -> Option<LockedPackage> {
        let index = self
            .packages
            .iter()
            .position(|package| package.project_id == project_id)?;
        Some(self.packages.remove(index))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub source: String,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub kind: ContentKind,
    pub loader: Option<String>,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
    pub installed_path: PathBuf,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub installed_as_dependency: bool,
}

pub fn lock_file(server_dir: &Path) -> PathBuf {
    minecli_dir(server_dir).join(LOCK_FILE)
}

pub fn load_lockfile(server_dir: &Path) -> Result<LockFile> {
    let path = lock_file(server_dir);
    match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).map_err(|source| MinecliError::TomlDeserialize {
            path: path.clone(),
            source,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LockFile::default()),
        Err(error) => Err(MinecliError::Io {
            path,
            source: error,
        }),
    }
}

pub fn write_lockfile(server_dir: &Path, lockfile: &LockFile) -> Result<()> {
    let path = lock_file(server_dir);
    fs::create_dir_all(minecli_dir(server_dir)).at(minecli_dir(server_dir))?;
    let contents =
        toml::to_string_pretty(lockfile).map_err(|source| MinecliError::TomlSerialize {
            path: path.clone(),
            source,
        })?;
    write_atomic(&path, contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::core::lockfile::{LockFile, LockedPackage, load_lockfile, write_lockfile};
    use crate::core::server::ContentKind;

    #[test]
    fn round_trips_lockfile() {
        let temp = tempfile::tempdir().unwrap();
        let mut lockfile = LockFile::default();
        lockfile.upsert_package(LockedPackage {
            source: "modrinth".to_owned(),
            project_id: "P7dR8mSH".to_owned(),
            slug: "fabric-api".to_owned(),
            title: "Fabric API".to_owned(),
            kind: ContentKind::Mod,
            loader: Some("fabric".to_owned()),
            version_id: "version-id".to_owned(),
            version_number: "1.0.0".to_owned(),
            filename: "fabric-api.jar".to_owned(),
            hashes: BTreeMap::new(),
            installed_path: PathBuf::from("mods/fabric-api.jar"),
            dependencies: vec![],
            installed_as_dependency: false,
        });

        write_lockfile(temp.path(), &lockfile).unwrap();
        let loaded = load_lockfile(temp.path()).unwrap();

        assert_eq!(loaded, lockfile);
    }
}
