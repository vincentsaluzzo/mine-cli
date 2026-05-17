use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::manifest::{minecli_dir, write_atomic};
use crate::core::server::ContentKind;
use crate::error::{IoResultExt, MinecliError, Result};
use crate::sources::{source_identity, source_priority};

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

    #[allow(dead_code)]
    pub fn package_by_source_id(&self, source: &str, project_id: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|package| {
            package.source == source && package.source_project_id_or_project_id() == project_id
        })
    }

    pub fn package_by_query(&self, query: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|package| {
            package.slug == query
                || package.project_id == query
                || package.source_identity() == query
                || package.source_version_id_or_version_id() == query
                || package.filename == query
                || package.installed_path == Path::new(query)
        })
    }

    pub fn upsert_package(&mut self, package: LockedPackage) {
        if let Some(existing) = self
            .packages
            .iter_mut()
            .find(|existing| existing.same_source_project(&package))
        {
            *existing = package;
        } else {
            self.packages.push(package);
        }
        self.packages.sort_by(|left, right| {
            source_priority(&left.source)
                .cmp(&source_priority(&right.source))
                .then_with(|| left.slug.cmp(&right.slug))
        });
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project_id: Option<String>,
    pub slug: String,
    pub title: String,
    pub kind: ContentKind,
    pub loader: Option<String>,
    pub version_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version_id: Option<String>,
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

impl LockedPackage {
    pub fn source_project_id_or_project_id(&self) -> &str {
        self.source_project_id
            .as_deref()
            .unwrap_or(&self.project_id)
    }

    pub fn source_version_id_or_version_id(&self) -> &str {
        self.source_version_id
            .as_deref()
            .unwrap_or(&self.version_id)
    }

    pub fn source_identity(&self) -> String {
        source_identity(&self.source, self.source_project_id_or_project_id())
    }

    pub fn same_source_project(&self, other: &Self) -> bool {
        self.source == other.source
            && self.source_project_id_or_project_id() == other.source_project_id_or_project_id()
    }
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
            source_project_id: Some("P7dR8mSH".to_owned()),
            slug: "fabric-api".to_owned(),
            title: "Fabric API".to_owned(),
            kind: ContentKind::Mod,
            loader: Some("fabric".to_owned()),
            version_id: "version-id".to_owned(),
            source_version_id: Some("version-id".to_owned()),
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

    #[test]
    fn upsert_uses_source_specific_project_identity() {
        let mut modrinth = package("modrinth", "same-id", "modrinth-version");
        modrinth.slug = "modrinth-package".to_owned();
        let mut hangar = package("hangar", "same-id", "hangar-version");
        hangar.slug = "hangar-package".to_owned();
        let mut lockfile = LockFile::default();

        lockfile.upsert_package(hangar);
        lockfile.upsert_package(modrinth);

        assert_eq!(lockfile.packages.len(), 2);
        assert!(lockfile.package_by_source_id("hangar", "same-id").is_some());
        assert!(
            lockfile
                .package_by_source_id("modrinth", "same-id")
                .is_some()
        );
        assert_eq!(lockfile.packages[0].source, "modrinth");
        assert_eq!(lockfile.packages[1].source, "hangar");
    }

    #[test]
    fn package_query_accepts_source_qualified_identity() {
        let package = package("hangar", "same-id", "1.0.0");
        let lockfile = LockFile {
            packages: vec![package],
        };

        assert!(lockfile.package_by_query("hangar:same-id").is_some());
    }

    #[test]
    fn handles_large_lockfiles_without_losing_entries() {
        let temp = tempfile::tempdir().unwrap();
        let packages = (0..2_000)
            .map(|index| package("modrinth", &format!("project-{index}"), "1.0.0"))
            .collect::<Vec<_>>();
        let lockfile = LockFile { packages };

        write_lockfile(temp.path(), &lockfile).unwrap();
        let loaded = load_lockfile(temp.path()).unwrap();

        assert_eq!(loaded.packages.len(), 2_000);
        assert!(loaded.package_by_query("project-1999").is_some());
    }

    fn package(source: &str, project_id: &str, version_id: &str) -> LockedPackage {
        LockedPackage {
            source: source.to_owned(),
            project_id: project_id.to_owned(),
            source_project_id: Some(project_id.to_owned()),
            slug: project_id.to_owned(),
            title: project_id.to_owned(),
            kind: ContentKind::Mod,
            loader: Some("fabric".to_owned()),
            version_id: version_id.to_owned(),
            source_version_id: Some(version_id.to_owned()),
            version_number: "1.0.0".to_owned(),
            filename: format!("{project_id}.jar"),
            hashes: BTreeMap::new(),
            installed_path: PathBuf::from(format!("mods/{project_id}.jar")),
            dependencies: vec![],
            installed_as_dependency: false,
        }
    }
}
