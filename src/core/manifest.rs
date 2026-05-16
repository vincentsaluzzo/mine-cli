use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::server::{ServerPaths, ServerType};
use crate::error::{IoResultExt, MinecliError, Result};

pub const MINECLI_DIR: &str = ".minecli";
pub const SERVER_FILE: &str = "server.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub minecraft_version: String,
    pub server_type: ServerType,
    pub world: String,
    pub paths: ServerPaths,
}

impl ServerConfig {
    pub fn new(
        name: String,
        minecraft_version: String,
        server_type: ServerType,
        world: String,
    ) -> Self {
        let paths = ServerPaths::defaults(&world);
        Self {
            name,
            minecraft_version,
            server_type,
            world,
            paths,
        }
    }
}

pub fn minecli_dir(server_dir: &Path) -> PathBuf {
    server_dir.join(MINECLI_DIR)
}

pub fn server_file(server_dir: &Path) -> PathBuf {
    minecli_dir(server_dir).join(SERVER_FILE)
}

pub fn load_server_config(server_dir: &Path) -> Result<ServerConfig> {
    let path = server_file(server_dir);
    let contents = fs::read_to_string(&path).at(&path)?;
    toml::from_str(&contents).map_err(|source| MinecliError::TomlDeserialize { path, source })
}

pub fn write_server_config(server_dir: &Path, config: &ServerConfig) -> Result<()> {
    let path = server_file(server_dir);
    fs::create_dir_all(minecli_dir(server_dir)).at(minecli_dir(server_dir))?;
    let contents =
        toml::to_string_pretty(config).map_err(|source| MinecliError::TomlSerialize {
            path: path.clone(),
            source,
        })?;
    write_atomic(&path, contents.as_bytes())
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        MinecliError::message(format!(
            "cannot write {} without a parent directory",
            path.display()
        ))
    })?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    fs::create_dir_all(parent).at(parent)?;

    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("minecli"),
        std::process::id()
    ));
    fs::write(&temp_path, contents).at(&temp_path)?;
    fs::rename(&temp_path, path).at(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::core::manifest::{ServerConfig, load_server_config, write_server_config};
    use crate::core::server::ServerType;

    #[test]
    fn round_trips_server_config() {
        let temp = tempfile::tempdir().unwrap();
        let config = ServerConfig::new(
            "survival".to_owned(),
            "1.21.5".to_owned(),
            ServerType::Fabric,
            "world".to_owned(),
        );

        write_server_config(temp.path(), &config).unwrap();
        let loaded = load_server_config(temp.path()).unwrap();

        assert_eq!(loaded, config);
    }
}
