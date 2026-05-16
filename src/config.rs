use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{IoResultExt, MinecliError, Result};

pub const CONFIG_FILE: &str = "config.toml";
pub const SERVERS_FILE: &str = "servers.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub default_server: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerRegistry {
    #[serde(default)]
    pub servers: BTreeMap<String, RegisteredServer>,
}

impl ServerRegistry {
    pub fn add(&mut self, name: String, path: PathBuf) -> Result<()> {
        if self.servers.contains_key(&name) {
            return Err(MinecliError::message(format!(
                "server `{name}` is already registered"
            )));
        }
        self.servers.insert(name, RegisteredServer { path });
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<RegisteredServer> {
        self.servers
            .remove(name)
            .ok_or_else(|| MinecliError::message(format!("server `{name}` is not registered")))
    }

    pub fn get(&self, name: &str) -> Result<&RegisteredServer> {
        self.servers
            .get(name)
            .ok_or_else(|| MinecliError::message(format!("server `{name}` is not registered")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredServer {
    pub path: PathBuf,
}

pub fn config_dir(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path.to_path_buf());
    }

    if let Some(path) = std::env::var_os("MINECLI_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }

    ProjectDirs::from("", "", "minecli")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or_else(|| MinecliError::message("could not resolve user config directory"))
}

pub fn config_file(config_dir: &Path) -> PathBuf {
    config_dir.join(CONFIG_FILE)
}

pub fn servers_file(config_dir: &Path) -> PathBuf {
    config_dir.join(SERVERS_FILE)
}

pub fn load_global_config(config_dir: &Path) -> Result<GlobalConfig> {
    load_toml_or_default(&config_file(config_dir))
}

pub fn write_global_config(config_dir: &Path, config: &GlobalConfig) -> Result<()> {
    write_toml(config_dir, &config_file(config_dir), config)
}

pub fn load_server_registry(config_dir: &Path) -> Result<ServerRegistry> {
    load_toml_or_default(&servers_file(config_dir))
}

pub fn write_server_registry(config_dir: &Path, registry: &ServerRegistry) -> Result<()> {
    write_toml(config_dir, &servers_file(config_dir), registry)
}

fn load_toml_or_default<T>(path: &Path) -> Result<T>
where
    T: Default + for<'de> Deserialize<'de>,
{
    match fs::read_to_string(path) {
        Ok(contents) => toml::from_str(&contents).map_err(|source| MinecliError::TomlDeserialize {
            path: path.to_path_buf(),
            source,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(MinecliError::Io {
            path: path.to_path_buf(),
            source: error,
        }),
    }
}

fn write_toml<T>(config_dir: &Path, path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    fs::create_dir_all(config_dir).at(config_dir)?;
    let contents = toml::to_string_pretty(value).map_err(|source| MinecliError::TomlSerialize {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, contents).at(path)
}

#[cfg(test)]
mod tests {
    use crate::config::{ServerRegistry, config_dir, load_server_registry, write_server_registry};

    #[test]
    fn round_trips_server_registry() {
        let temp = tempfile::tempdir().unwrap();
        let mut registry = ServerRegistry::default();
        registry
            .add("survival".to_owned(), "/srv/minecraft/survival".into())
            .unwrap();

        write_server_registry(temp.path(), &registry).unwrap();
        let loaded = load_server_registry(temp.path()).unwrap();

        assert_eq!(loaded, registry);
    }

    #[test]
    fn rejects_duplicate_servers() {
        let mut registry = ServerRegistry::default();
        registry.add("survival".to_owned(), "/one".into()).unwrap();

        let result = registry.add("survival".to_owned(), "/two".into());

        assert!(result.is_err());
    }

    #[test]
    fn uses_explicit_config_dir_override() {
        let path = std::path::Path::new("/tmp/minecli-test-config");

        assert_eq!(config_dir(Some(path)).unwrap(), path);
    }
}
